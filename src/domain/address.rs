// SPDX-License-Identifier: AGPL-3.0-only
//! Offline nearby address/street hints from separately licensed public OSM data.
use super::GeoPoint;
use serde::Deserialize;

type Point = [f64; 2];
#[derive(Deserialize)]
struct Dataset {
    license: String,
    houses: Vec<(f64, f64, String, String)>,
    roads: Vec<(String, Vec<Point>)>,
}
struct Road {
    name: String,
    points: Vec<Point>,
    bbox: [f64; 4],
}
/// Approximate nearby addresses, never a guessed cadastral/exact house assignment.
pub struct AddressMap {
    houses: Vec<(f64, f64, String, String)>,
    roads: Vec<Road>,
}
impl AddressMap {
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
            roads,
        })
    }
    /// Returns a nearby house within 60 m or a named street within 80 m.
    /// House centers are approximations; labels explicitly say "Рядом".
    #[must_use]
    pub fn resolve(&self, point: GeoPoint) -> Option<String> {
        GeoPoint::new(point.lat, point.lon)?;
        let scale = 111_195.0;
        let lon_scale = scale * point.lat.to_radians().cos();
        if lon_scale.abs() < 1.0 {
            return None;
        }
        let project = |p: Point| [(p[0] - point.lon) * lon_scale, (p[1] - point.lat) * scale];
        let lon_margin = 80.0 / lon_scale;
        let lat_margin = 80.0 / scale;
        let mut house = None;
        let mut house_distance = 60.0_f64;
        for entry in &self.houses {
            if (entry.0 - point.lon).abs() > lon_margin || (entry.1 - point.lat).abs() > lat_margin
            {
                continue;
            }
            let p = project([entry.0, entry.1]);
            let distance = p[0].hypot(p[1]);
            if distance < house_distance {
                house_distance = distance;
                house = Some(entry);
            }
        }
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
                let a = project(pair[0]);
                let b = project(pair[1]);
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let length2 = dx * dx + dy * dy;
                let t = if length2 > 0.0 {
                    (-(a[0] * dx + a[1] * dy) / length2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let distance = (a[0] + t * dx).hypot(a[1] + t * dy);
                if distance < street_distance {
                    street_distance = distance;
                    street = Some(&road.name);
                }
            }
        }
        if let Some((_, _, name, number)) = house {
            if house_distance <= 30.0 || street.is_none_or(|road| road == name) {
                return Some(format!("Рядом: {name}, д. {number}"));
            }
        }
        street.map(|name| format!("Рядом: {name}"))
    }
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
        assert!(map.houses.len() > 100_000);
        for point in [(59.9330, 30.3331), (59.9401, 30.3159), (59.9167, 30.3180)] {
            assert!(
                map.resolve(GeoPoint::new(point.0, point.1).unwrap())
                    .is_some(),
                "city landmark should have an address hint"
            );
        }
        assert_eq!(map.resolve(GeoPoint::new(55.75, 37.61).unwrap()), None);
    }
}
