// SPDX-License-Identifier: AGPL-3.0-only
//! Local district resolution. Boundary data is separately licensed ODbL-1.0.
use super::GeoPoint;
use serde::Deserialize;

#[derive(Deserialize)]
struct District {
    id: String,
    bbox: [f64; 4],
    outer: Vec<Vec<[f64; 2]>>,
    inner: Vec<Vec<[f64; 2]>>,
}

/// Immutable geographic lookup; no remote geocoder or personal-data persistence.
pub struct DistrictMap {
    districts: Vec<District>,
}

impl DistrictMap {
    /// Loads and validates the bundled OSM district boundaries.
    /// # Errors
    /// Returns an error for malformed or incomplete boundary assets.
    pub fn bundled() -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        struct Catalog {
            districts: Vec<District>,
        }
        let catalog: Catalog =
            serde_json::from_str(include_str!("../../assets/areas/spb-districts.json"))?;
        anyhow::ensure!(catalog.districts.len() == 18, "incomplete district catalog");
        let mut ids = std::collections::HashSet::new();
        for district in &catalog.districts {
            anyhow::ensure!(
                ids.insert(&district.id) && !district.outer.is_empty(),
                "invalid district identity"
            );
            let [west, south, east, north] = district.bbox;
            anyhow::ensure!(
                GeoPoint::new(south, west).is_some()
                    && GeoPoint::new(north, east).is_some()
                    && west < east
                    && south < north,
                "invalid district bounds"
            );
            for ring in district.outer.iter().chain(&district.inner) {
                anyhow::ensure!(
                    ring.len() >= 4 && ring.first() == ring.last(),
                    "unclosed district ring"
                );
                anyhow::ensure!(
                    ring.iter().all(|p| GeoPoint::new(p[1], p[0]).is_some()),
                    "invalid district geometry"
                );
                anyhow::ensure!(
                    ring.iter()
                        .all(|p| p[0] >= west && p[0] <= east && p[1] >= south && p[1] <= north),
                    "district bounds exclude geometry"
                );
            }
        }
        Ok(Self {
            districts: catalog.districts,
        })
    }

    /// Returns the containing district, never a guessed nearest district.
    /// Shared boundaries resolve deterministically in the dataset's ID order.
    #[must_use]
    pub fn district_at(&self, point: GeoPoint) -> Option<&str> {
        GeoPoint::new(point.lat, point.lon)?;
        self.districts
            .iter()
            .find(|district| {
                let [west, south, east, north] = district.bbox;
                point.lon >= west
                    && point.lon <= east
                    && point.lat >= south
                    && point.lat <= north
                    && district.outer.iter().any(|ring| contains(ring, point))
                    && !district.inner.iter().any(|ring| contains(ring, point))
            })
            .map(|district| district.id.as_str())
    }

    /// Districts possibly touched by Telegram's GPS uncertainty radius.
    /// Uses a local spherical projection with a conservative 5 m margin.
    /// This is advisory only: it never reassigns a point outside the city.
    /// Invalid or unsupported radii return no candidates.
    #[must_use]
    pub fn districts_near(&self, point: GeoPoint, radius_m: f64) -> Vec<&str> {
        if GeoPoint::new(point.lat, point.lon).is_none()
            || !radius_m.is_finite()
            || !(0.0..=1500.0).contains(&radius_m)
        {
            return Vec::new();
        }
        let radius = radius_m + 5.0;
        let lat_scale = 6_371_008.8 * std::f64::consts::PI / 180.0;
        let lon_scale = lat_scale * point.lat.to_radians().cos();
        let lat_margin = radius / lat_scale;
        let lon_margin = radius / lon_scale.max(1.0);
        self.districts
            .iter()
            .filter(|district| {
                let [west, south, east, north] = district.bbox;
                if point.lon < west - lon_margin
                    || point.lon > east + lon_margin
                    || point.lat < south - lat_margin
                    || point.lat > north + lat_margin
                {
                    return false;
                }
                (district.outer.iter().any(|ring| contains(ring, point))
                    && !district.inner.iter().any(|ring| contains(ring, point)))
                    || district.outer.iter().chain(&district.inner).any(|ring| {
                        ring.windows(2).any(|edge| {
                            segment_distance(point, edge, lon_scale, lat_scale) <= radius
                        })
                    })
            })
            .map(|district| district.id.as_str())
            .collect()
    }
}

fn segment_distance(point: GeoPoint, edge: &[[f64; 2]], lon_scale: f64, lat_scale: f64) -> f64 {
    let ax = (edge[0][0] - point.lon) * lon_scale;
    let ay = (edge[0][1] - point.lat) * lat_scale;
    let dx = (edge[1][0] - edge[0][0]) * lon_scale;
    let dy = (edge[1][1] - edge[0][1]) * lat_scale;
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (-(ax * dx + ay * dy) / length_squared).clamp(0.0, 1.0)
    };
    (ax + t * dx).hypot(ay + t * dy)
}

fn contains(ring: &[[f64; 2]], point: GeoPoint) -> bool {
    let (x, y) = (point.lon, point.lat);
    let mut inside = false;
    for edge in ring.windows(2) {
        let ([ax, ay], [bx, by]) = (edge[0], edge[1]);
        let cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax);
        if cross.abs() <= 1e-12 * (bx - ax).hypot(by - ay)
            && x >= ax.min(bx)
            && x <= ax.max(bx)
            && y >= ay.min(by)
            && y <= ay.max(by)
        {
            return true;
        }
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_landmarks_and_rejects_outside_city_or_invalid_points() {
        let map = DistrictMap::bundled().unwrap();
        for (lat, lon, district) in [
            (59.916, 30.292, "spb-admiralteysky"),
            (59.938, 30.35, "spb-tsentralny"),
            (59.943, 30.265, "spb-vasileostrovsky"),
            (59.965, 30.31, "spb-petrogradsky"),
            (59.72, 30.405, "spb-pushkinsky"),
            (59.75, 30.595, "spb-kolpinsky"),
            (59.995, 29.77, "spb-kronshtadtsky"),
            (60.055, 30.33, "spb-vyborgsky"),
            (59.999, 30.398, "spb-kalininsky"),
            (59.877, 30.264, "spb-kirovsky"),
            (59.966, 30.432, "spb-krasnogvardeysky"),
            (59.735, 30.095, "spb-krasnoselsky"),
            (60.097, 29.956, "spb-kurortny"),
            (59.849, 30.322, "spb-moskovsky"),
            (59.881, 30.448, "spb-nevsky"),
            (59.883, 29.905, "spb-petrodvortsovy"),
            (60.007, 30.224, "spb-primorsky"),
            (59.857, 30.408, "spb-frunzensky"),
        ] {
            assert_eq!(
                map.district_at(GeoPoint::new(lat, lon).unwrap()),
                Some(district)
            );
        }
        assert_eq!(map.district_at(GeoPoint::new(55.75, 37.62).unwrap()), None);
        assert_eq!(map.district_at(GeoPoint::new(60.1, 30.65).unwrap()), None);
        assert_eq!(
            map.district_at(GeoPoint {
                lat: f64::NAN,
                lon: 30.3
            }),
            None
        );
    }
    #[test]
    fn uncertainty_checks_neighbors_without_guessing_a_nearest_area() {
        let rectangle = |id: &str, west, east| District {
            id: id.into(),
            bbox: [west, 0.0, east, 0.01],
            outer: vec![vec![
                [west, 0.0],
                [east, 0.0],
                [east, 0.01],
                [west, 0.01],
                [west, 0.0],
            ]],
            inner: vec![],
        };
        let map = DistrictMap {
            districts: vec![rectangle("west", 0.0, 0.01), rectangle("east", 0.01, 0.02)],
        };
        let point = GeoPoint::new(0.005, 0.0095).unwrap();
        assert_eq!(map.district_at(point), Some("west"));
        assert_eq!(map.districts_near(point, 10.0), vec!["west"]);
        assert_eq!(map.districts_near(point, 100.0), vec!["west", "east"]);
        assert_eq!(
            map.district_at(GeoPoint::new(0.005, 0.01).unwrap()),
            Some("west")
        );
        assert_eq!(
            map.districts_near(GeoPoint::new(0.005, 0.03).unwrap(), 100.0),
            Vec::<&str>::new()
        );
        assert_eq!(map.districts_near(point, -1.0), Vec::<&str>::new());
        assert_eq!(map.districts_near(point, f64::NAN), Vec::<&str>::new());
    }
    #[test]
    fn ring_edges_and_holes_do_not_become_nearest_districts() {
        let map = DistrictMap {
            districts: vec![District {
                id: "test".into(),
                bbox: [0.0, 0.0, 10.0, 10.0],
                outer: vec![vec![
                    [0.0, 0.0],
                    [10.0, 0.0],
                    [10.0, 10.0],
                    [0.0, 10.0],
                    [0.0, 0.0],
                ]],
                inner: vec![vec![
                    [3.0, 3.0],
                    [7.0, 3.0],
                    [7.0, 7.0],
                    [3.0, 7.0],
                    [3.0, 3.0],
                ]],
            }],
        };
        assert_eq!(
            map.district_at(GeoPoint::new(0.0, 5.0).unwrap()),
            Some("test")
        );
        assert_eq!(map.district_at(GeoPoint::new(5.0, 5.0).unwrap()), None);
        let near_hole = GeoPoint::new(3.001, 5.0).unwrap();
        assert_eq!(map.district_at(near_hole), None);
        assert_eq!(map.districts_near(near_hole, 10.0), Vec::<&str>::new());
        assert_eq!(map.districts_near(near_hole, 200.0), vec!["test"]);
        assert_eq!(map.district_at(GeoPoint::new(5.0, 11.0).unwrap()), None);
    }
}
