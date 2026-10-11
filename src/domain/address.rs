// SPDX-License-Identifier: AGPL-3.0-only
//! Offline nearby address/street hints from separately licensed public OSM data.
use super::area::{contains, segment_distance};
use super::GeoPoint;
use serde::Deserialize;

type Point = [f64; 2];
type BuildingData = (String, String, Vec<Point>, Vec<Vec<Point>>);
/// A public address search result. The point identifies a building, not a person.
#[derive(Clone, Debug)]
pub struct AddressCandidate {
    pub label: String,
    pub point: GeoPoint,
}
#[derive(Deserialize)]
struct Dataset {
    license: String,
    houses: Vec<(f64, f64, String, String)>,
    #[serde(default)]
    buildings: Vec<BuildingData>,
    roads: Vec<(String, Vec<Point>)>,
}
struct Road {
    name: String,
    points: Vec<Point>,
    bbox: [f64; 4],
}
struct Building {
    street: String,
    number: String,
    outer: Vec<Point>,
    inner: Vec<Vec<Point>>,
    bbox: [f64; 4],
}
/// Approximate nearby addresses, never a guessed cadastral/exact house assignment.
pub struct AddressMap {
    houses: Vec<(f64, f64, String, String)>,
    buildings: Vec<Building>,
    roads: Vec<Road>,
}
impl AddressMap {
    /// Searches local street/house/corpus labels; at most six unique addresses.
    /// An address must include a street and house number. No external geocoder.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<AddressCandidate> {
        let query = query.to_lowercase().replace('ё', "е");
        let parses: Vec<_> = query
            .char_indices()
            .filter(|(index, c)| {
                c.is_ascii_digit()
                    && (*index == 0 || !query[..*index].ends_with(|c: char| c.is_ascii_digit()))
            })
            .map(|(index, _)| (street_words(&query[..index]), house_number(&query[index..])))
            .filter(|(words, number)| !words.is_empty() && !number.is_empty())
            .collect();
        if parses.is_empty() {
            return Vec::new();
        }
        let matches = |street: &str, house: &str| {
            let street = street_words(street).join(" ");
            let house = house_number(house);
            parses.iter().any(|(words, number)| {
                words
                    .iter()
                    .all(|word| street.split_whitespace().any(|part| part == word))
                    && house.strip_prefix(number.as_str()).is_some_and(|rest| {
                        rest.is_empty()
                            || number.ends_with('к')
                            || !rest.starts_with(|c: char| c.is_ascii_digit())
                    })
            })
        };
        let mut results = std::collections::BTreeMap::new();
        for building in &self.buildings {
            if matches(&building.street, &building.number) {
                if let Some(point) = building.interior_point() {
                    results
                        .entry(format!("{}, д. {}", building.street, building.number))
                        .or_insert(point);
                }
            }
        }
        for (lon, lat, street, number) in &self.houses {
            if matches(street, number) {
                results
                    .entry(format!("{street}, д. {number}"))
                    .or_insert(GeoPoint {
                        lat: *lat,
                        lon: *lon,
                    });
            }
        }
        // Exact corpus/letter matches win over broader prefixes; retain ambiguity
        // between streets with the same short name so the person must choose.
        let exact: Vec<_> = results
            .iter()
            .filter(|(label, _)| {
                label.rsplit_once(", д. ").is_some_and(|(_, n)| {
                    parses.iter().any(|(_, number)| house_number(n) == *number)
                })
            })
            .map(|(label, point)| AddressCandidate {
                label: label.clone(),
                point: *point,
            })
            .collect();
        if !exact.is_empty() {
            return exact.into_iter().take(6).collect();
        }
        results
            .into_iter()
            .take(6)
            .map(|(label, point)| AddressCandidate { label, point })
            .collect()
    }
    /// Loads the public city snapshot embedded in the binary. No runtime network.
    /// # Errors
    /// Returns an error for an incomplete or invalid address/road dataset.
    pub fn bundled() -> anyhow::Result<Self> {
        Self::from_json(include_str!("../../assets/areas/spb-addresses.json"))
    }
    fn from_json(raw: &str) -> anyhow::Result<Self> {
        let data: Dataset = serde_json::from_str(raw)?;
        anyhow::ensure!(
            data.license == "ODbL-1.0" && !data.houses.is_empty() && !data.roads.is_empty(),
            "incomplete address data"
        );
        for (lon, lat, street, number) in &data.houses {
            anyhow::ensure!(
                GeoPoint::new(*lat, *lon).is_some()
                    && !street.trim().is_empty()
                    && !number.trim().is_empty(),
                "invalid address point"
            );
        }
        let mut buildings = Vec::with_capacity(data.buildings.len());
        for (street, number, outer, inner) in data.buildings {
            anyhow::ensure!(
                !street.trim().is_empty()
                    && !number.trim().is_empty()
                    && std::iter::once(&outer).chain(&inner).all(|ring| {
                        ring.len() >= 4
                            && ring.first() == ring.last()
                            && ring.iter().all(|p| GeoPoint::new(p[1], p[0]).is_some())
                    }),
                "invalid building geometry"
            );
            let bbox = bounds(&outer);
            anyhow::ensure!(
                inner.iter().flatten().all(|p| p[0] >= bbox[0]
                    && p[0] <= bbox[2]
                    && p[1] >= bbox[1]
                    && p[1] <= bbox[3]),
                "building bounds exclude courtyard"
            );
            buildings.push(Building {
                street,
                number,
                outer,
                inner,
                bbox,
            });
        }
        let mut roads = Vec::with_capacity(data.roads.len());
        for (name, points) in data.roads {
            anyhow::ensure!(
                !name.trim().is_empty()
                    && points.len() >= 2
                    && points.iter().all(|p| GeoPoint::new(p[1], p[0]).is_some()),
                "invalid street geometry"
            );
            let mut bbox = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            for p in &points {
                bbox[0] = bbox[0].min(p[0]);
                bbox[1] = bbox[1].min(p[1]);
                bbox[2] = bbox[2].max(p[0]);
                bbox[3] = bbox[3].max(p[1]);
            }
            roads.push(Road { name, points, bbox });
        }
        Ok(Self {
            houses: data.houses,
            buildings,
            roads,
        })
    }
    /// Returns an unambiguous nearby building within 20 m, or a street within 80 m.
    /// Building outlines and courtyard exclusions take precedence over centers.
    #[must_use]
    pub fn resolve(&self, point: GeoPoint) -> Option<String> {
        self.resolve_with_accuracy(point, None)
    }
    /// Accounts for Telegram's optional GPS uncertainty without persisting it.
    /// A radius above 25 m or two plausible houses suppresses the house number.
    #[must_use]
    pub fn resolve_with_accuracy(&self, point: GeoPoint, accuracy: Option<f64>) -> Option<String> {
        GeoPoint::new(point.lat, point.lon)?;
        let scale = 111_195.0;
        let lon_scale = scale * point.lat.to_radians().cos();
        if lon_scale.abs() < 1.0 {
            return None;
        }
        let lon_margin = 80.0 / lon_scale;
        let lat_margin = 80.0 / scale;
        let mut candidates = Vec::new();
        for entry in &self.houses {
            if (entry.0 - point.lon).abs() > lon_margin || (entry.1 - point.lat).abs() > lat_margin
            {
                continue;
            }
            let distance = ((entry.0 - point.lon) * lon_scale).hypot((entry.1 - point.lat) * scale);
            if distance <= 80.0 {
                candidates.push((distance, entry.2.as_str(), entry.3.as_str()));
            }
        }
        for building in &self.buildings {
            if !near_bounds(point, building.bbox, lon_margin, lat_margin) {
                continue;
            }
            let distance = if contains(&building.outer, point)
                && !building.inner.iter().any(|ring| contains(ring, point))
            {
                0.0
            } else {
                std::iter::once(&building.outer)
                    .chain(&building.inner)
                    .flat_map(|ring| ring.windows(2))
                    .map(|edge| segment_distance(point, edge, lon_scale, scale))
                    .fold(f64::INFINITY, f64::min)
            };
            if distance <= 80.0 {
                candidates.push((distance, building.street.as_str(), building.number.as_str()));
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(b.1)).then(a.2.cmp(b.2)));
        let mut street = None;
        let mut street_distance = 80.0_f64;
        for road in &self.roads {
            if point.lon < road.bbox[0] - lon_margin
                || point.lon > road.bbox[2] + lon_margin
                || point.lat < road.bbox[1] - lat_margin
                || point.lat > road.bbox[3] + lat_margin
            {
                continue;
            }
            for pair in road.points.windows(2) {
                let distance = segment_distance(point, pair, lon_scale, scale);
                if distance < street_distance {
                    street_distance = distance;
                    street = Some(&road.name);
                }
            }
        }
        if let Some(&(distance, name, number)) = candidates.first() {
            let radius = accuracy.filter(|a| a.is_finite() && (0.0..=1500.0).contains(a));
            let uncertain =
                accuracy.is_some() && radius.is_none() || radius.is_some_and(|r| r > 25.0);
            let runner_up = candidates.iter().find(|c| c.1 != name || c.2 != number);
            let separated = runner_up.is_none_or(|other| {
                other.0 - distance >= 15.0_f64.max(2.0 * radius.unwrap_or(0.0))
            });
            if !uncertain
                && separated
                && distance <= 20.0
                && (distance <= 5.0 || street.is_none_or(|road| road == name))
            {
                return Some(format!("Рядом: {name}, д. {number}"));
            }
            return Some(format!("Рядом: {}", street.map_or(name, String::as_str)));
        }
        street.map(|name| format!("Рядом: {name}"))
    }
}

impl Building {
    fn interior_point(&self) -> Option<GeoPoint> {
        let [west, south, east, north] = self.bbox;
        for (x, y) in std::iter::once((0.5, 0.5)).chain(
            (1..10).flat_map(|x| (1..10).map(move |y| (f64::from(x) / 10.0, f64::from(y) / 10.0))),
        ) {
            let point = GeoPoint {
                lon: west + x * (east - west),
                lat: south + y * (north - south),
            };
            if contains(&self.outer, point) && !self.inner.iter().any(|ring| contains(ring, point))
            {
                return Some(point);
            }
        }
        None
    }
}
fn street_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('ё', "е")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| {
            !word.is_empty()
                && ![
                    "санкт",
                    "петербург",
                    "спб",
                    "улица",
                    "ул",
                    "проспект",
                    "просп",
                    "пр",
                    "проезд",
                    "переулок",
                    "пер",
                    "дом",
                    "д",
                ]
                .contains(word)
        })
        .map(str::to_owned)
        .collect()
}
fn house_number(text: &str) -> String {
    text.to_lowercase()
        .replace("корпус", "к")
        .replace("корп", "к")
        .replace("литера", "лит")
        .replace('k', "к")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '/')
        .collect()
}

fn bounds(points: &[Point]) -> [f64; 4] {
    points.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |mut bbox, p| {
            bbox[0] = bbox[0].min(p[0]);
            bbox[1] = bbox[1].min(p[1]);
            bbox[2] = bbox[2].max(p[0]);
            bbox[3] = bbox[3].max(p[1]);
            bbox
        },
    )
}
fn near_bounds(point: GeoPoint, bbox: [f64; 4], lon: f64, lat: f64) -> bool {
    point.lon >= bbox[0] - lon
        && point.lon <= bbox[2] + lon
        && point.lat >= bbox[1] - lat
        && point.lat <= bbox[3] + lat
}

#[cfg(test)]
mod tests {
    use super::*;
    const DATA: &str = r#"{"license":"ODbL-1.0","houses":[[30.0,60.0,"Первая улица","10"]],"roads":[["Первая улица",[[29.99,60.0],[30.01,60.0]]],["Вторая улица",[[29.99,60.001],[30.01,60.001]]]]}"#;
    #[test]
    fn hints_use_houses_and_road_segments_with_bounded_distance() {
        let map = AddressMap::from_json(DATA).unwrap();
        assert_eq!(
            map.resolve(GeoPoint::new(60.0, 30.0).unwrap()).as_deref(),
            Some("Рядом: Первая улица, д. 10")
        );
        assert_eq!(
            map.resolve(GeoPoint::new(60.001, 30.004).unwrap())
                .as_deref(),
            Some("Рядом: Вторая улица")
        );
        assert_eq!(map.resolve(GeoPoint::new(60.004, 30.0).unwrap()), None);
        assert_eq!(
            map.resolve(GeoPoint {
                lat: f64::NAN,
                lon: 30.0
            }),
            None
        );
    }
    #[test]
    fn invalid_data_is_rejected() {
        assert!(AddressMap::from_json(&DATA.replace("[29.99,60.0]", "[299.99,60.0]")).is_err());
        assert!(AddressMap::from_json(&DATA.replace("ODbL-1.0", "unknown")).is_err());
    }
    #[test]
    fn bundled_city_addresses_cover_known_landmarks() {
        let map = AddressMap::bundled().unwrap();
        assert!(map.houses.len() > 9_000);
        assert!(map.buildings.len() > 100_000);
        for point in [(59.9330, 30.3331), (59.9401, 30.3159), (59.9167, 30.3180)] {
            assert!(
                map.resolve(GeoPoint::new(point.0, point.1).unwrap())
                    .is_some(),
                "city landmark should have an address hint"
            );
        }
        assert_eq!(map.resolve(GeoPoint::new(55.75, 37.61).unwrap()), None);
    }
    #[test]
    fn long_building_outline_wins_over_nearer_neighbor_center() {
        let data = r#"{"license":"ODbL-1.0","houses":[[30.00015,60.0,"Улица","10"]],"buildings":[["Улица","17 к3",[[29.998,59.99995],[30.0,59.99995],[30.0,60.00005],[29.998,60.00005],[29.998,59.99995]],[]]],"roads":[["Улица",[[29.99,60.0005],[30.01,60.0005]]]]}"#;
        let map = AddressMap::from_json(data).unwrap();
        assert_eq!(
            map.resolve(GeoPoint::new(60.0, 29.99999).unwrap())
                .as_deref(),
            Some("Рядом: Улица")
        );
        // A closer centroid would choose 10. Distinct alternatives only 9 m apart
        // are ambiguous: neither house number is asserted.
        let map = AddressMap::from_json(&data.replace("30.00015", "30.0006")).unwrap();
        assert_eq!(
            map.resolve(GeoPoint::new(60.0, 29.99999).unwrap())
                .as_deref(),
            Some("Рядом: Улица, д. 17 к3")
        );
    }
    #[test]
    fn uncertain_gps_and_competing_addresses_suppress_house_numbers() {
        let map = AddressMap::from_json(DATA).unwrap();
        let point = GeoPoint::new(60.0, 30.0).unwrap();
        for accuracy in [30.0, 1500.0, f64::NAN, -1.0] {
            assert_eq!(
                map.resolve_with_accuracy(point, Some(accuracy)).as_deref(),
                Some("Рядом: Первая улица")
            );
        }
        assert_eq!(
            map.resolve_with_accuracy(point, Some(5.0)).as_deref(),
            Some("Рядом: Первая улица, д. 10")
        );
        let ambiguous = DATA.replace(
            "[30.0,60.0,\"Первая улица\",\"10\"]",
            "[30.0,60.0,\"Первая улица\",\"10\"],[30.0001,60.0,\"Первая улица\",\"12\"]",
        );
        assert_eq!(
            AddressMap::from_json(&ambiguous)
                .unwrap()
                .resolve(point)
                .as_deref(),
            Some("Рядом: Первая улица")
        );
    }
    #[test]
    fn dachny_corpus_uses_public_building_outline() {
        let map = AddressMap::bundled().unwrap();
        assert_eq!(
            map.resolve(GeoPoint::new(59.843_969, 30.249_835).unwrap())
                .as_deref(),
            Some("Рядом: Дачный проспект, д. 17 к3")
        );
        assert_eq!(
            map.resolve_with_accuracy(GeoPoint::new(59.843_969, 30.249_835).unwrap(), Some(80.0))
                .as_deref(),
            Some("Рядом: Дачный проспект")
        );
    }
    #[test]
    fn manual_search_requires_a_street_and_preserves_corpus_and_slash() {
        let map = AddressMap::bundled().unwrap();
        for query in [
            "Дачный проспект, дом 17 корпус 3",
            "Дачный, 17к3",
            "СПб, Дачный пр., д. 17 к 3",
        ] {
            let found = map.search(query);
            assert_eq!(found.len(), 1, "{query}");
            assert_eq!(found[0].label, "Дачный проспект, д. 17 к3");
            assert_eq!(
                map.resolve(found[0].point).as_deref(),
                Some("Рядом: Дачный проспект, д. 17 к3")
            );
        }
        assert_eq!(
            map.search("Дачный 10/7")[0].label,
            "Дачный проспект, д. 10/7"
        );
        for query in ["17", "Дачный", "несуществующая улица 12345"] {
            assert!(map.search(query).is_empty());
        }
    }
    #[test]
    fn search_retains_ambiguity_and_invalid_footprints_are_rejected() {
        let data = DATA.replace("[30.0,60.0,\"Первая улица\",\"10\"]", "[30.0,60.0,\"Первая улица\",\"10\"],[30.005,60.0,\"Первая улица\",\"10 к1\"],[30.006,60.0,\"Первая улица\",\"10 к2\"]");
        assert_eq!(
            AddressMap::from_json(&data)
                .unwrap()
                .search("Первая 10 к")
                .len(),
            2
        );
        assert_eq!(
            AddressMap::from_json(&data)
                .unwrap()
                .search("Первая 10 к2")
                .len(),
            1
        );
        let invalid = DATA.replace(
            "\"roads\":",
            "\"buildings\":[[\"Улица\",\"1\",[[30,60],[31,60],[31,61]],[]]],\"roads\":",
        );
        assert!(AddressMap::from_json(&invalid).is_err());
        let numbered = DATA.replace("Первая улица", "1-я Первая улица");
        assert_eq!(
            AddressMap::from_json(&numbered)
                .unwrap()
                .search("1-я Первая, 10")
                .len(),
            1
        );
    }
    #[test]
    fn courtyard_is_not_treated_as_building_interior() {
        let data = r#"{"license":"ODbL-1.0","houses":[[31,61,"Первая улица","2"]],"buildings":[["Первая улица","1",[[29.998,59.998],[30.002,59.998],[30.002,60.002],[29.998,60.002],[29.998,59.998]],[[[29.999,59.999],[30.001,59.999],[30.001,60.001],[29.999,60.001],[29.999,59.999]]]]],"roads":[["Первая улица",[[29.99,60],[30.01,60]]]]}"#;
        let map = AddressMap::from_json(data).unwrap();
        assert_eq!(
            map.resolve(GeoPoint::new(60.0, 30.0).unwrap()).as_deref(),
            Some("Рядом: Первая улица")
        );
        let found = map.search("Первая 1");
        // The geometric search result itself avoids the courtyard hole.
        assert_eq!(found.len(), 1);
        assert_eq!(
            map.resolve(found[0].point).as_deref(),
            Some("Рядом: Первая улица, д. 1")
        );
    }
}
