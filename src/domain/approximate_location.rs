// SPDX-License-Identifier: AGPL-3.0-only
//! Stable coarse location disclosure for Saint Petersburg invitations.
use super::GeoPoint;

// A fixed reference latitude is intentional: a scale derived from the exact
// latitude would disclose extra information and vary between recipients.
const LAT_METERS_PER_DEGREE: f64 = 111_195.0;
const LON_METERS_PER_DEGREE: f64 = LAT_METERS_PER_DEGREE * 0.5;
const CELL_METERS: f64 = 1_000.0;

/// A grid-cell center, not the requester point or a meeting place.
/// The grid is approximately 1 km square at Saint Petersburg's latitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApproximateLocation {
    pub center: GeoPoint,
    /// A fixed enclosing uncertainty radius; never the user's GPS accuracy.
    pub radius_meters: u16,
}
impl ApproximateLocation {
    /// Coarsens a point using a fixed grid. Identical cells always yield the
    /// same marker, across recipients, repeated renders, waves and restarts.
    /// No address, random jitter, heading or exact-distance metadata is emitted.
    #[must_use]
    pub fn from_point(point: GeoPoint) -> Option<Self> {
        GeoPoint::new(point.lat, point.lon)?;
        let center = |coordinate: f64, scale: f64| {
            ((coordinate * scale / CELL_METERS).floor() + 0.5) * CELL_METERS / scale
        };
        Some(Self {
            center: GeoPoint::new(
                center(point.lat, LAT_METERS_PER_DEGREE),
                center(point.lon, LON_METERS_PER_DEGREE),
            )?,
            radius_meters: 800,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn points_in_one_cell_share_a_marker_and_fixed_uncertainty() {
        let original = GeoPoint::new(59.938, 30.35).unwrap();
        let zone = ApproximateLocation::from_point(original).unwrap();
        assert_ne!(zone.center, original);
        for north in [-499.0, -250.0, 0.0, 250.0, 499.0] {
            for east in [-499.0, -250.0, 0.0, 250.0, 499.0] {
                let point = GeoPoint::new(
                    zone.center.lat + north / LAT_METERS_PER_DEGREE,
                    zone.center.lon + east / LON_METERS_PER_DEGREE,
                )
                .unwrap();
                assert_eq!(ApproximateLocation::from_point(point), Some(zone));
            }
        }
        assert_eq!(zone.radius_meters, 800);
        let next = GeoPoint::new(
            zone.center.lat,
            zone.center.lon + 1_001.0 / LON_METERS_PER_DEGREE,
        )
        .unwrap();
        assert_ne!(ApproximateLocation::from_point(next), Some(zone));
    }
    #[test]
    fn city_cell_corners_fit_the_fixed_uncertainty_radius() {
        for latitude in [59.6, 59.95, 60.3] {
            let zone =
                ApproximateLocation::from_point(GeoPoint::new(latitude, 30.35).unwrap()).unwrap();
            let lon_delta = 500.0 / LON_METERS_PER_DEGREE;
            let east = lon_delta * LAT_METERS_PER_DEGREE * zone.center.lat.to_radians().cos();
            assert!(east.hypot(500.0) < f64::from(zone.radius_meters));
        }
        for point in [
            GeoPoint {
                lat: f64::NAN,
                lon: 30.0,
            },
            GeoPoint {
                lat: 59.0,
                lon: f64::INFINITY,
            },
        ] {
            assert_eq!(ApproximateLocation::from_point(point), None);
        }
    }
}
