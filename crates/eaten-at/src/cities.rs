//! The towns the near line can name (plan 15): a `GeoNames` list of
//! places above 5,000 people, held in memory, with the region names
//! that label them. The Open Places API searches near a point and
//! cannot geocode, so a second point has to be ours: the author picks
//! a town, and the town's centroid is where the search looks.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context as _;

use crate::places::Point;

/// One town.
#[derive(Debug, Clone, PartialEq)]
pub struct City {
    /// The `GeoNames` id, what the form carries.
    pub id: u32,
    pub name: String,
    /// The name in ASCII, what typing is matched against.
    /// The name in ASCII, for slugs and matching.
    pub ascii: String,
    pub point: Point,
    /// ISO country code.
    country: String,
    /// The first-level region's code (`MA`, `ENG`) and name.
    region_code: Option<String>,
    region_name: Option<String>,
    population: u64,
}

impl City {
    /// How the near line says it: "Acton, MA" in the US, where the
    /// region code is what people say (`GeoNames` numbers the regions of
    /// Canada and Australia), "London, England" elsewhere, and no
    /// country (Ken, 2026-09-27).
    pub fn label(&self) -> String {
        let region = match self.country.as_str() {
            "US" => self.region_code.as_deref(),
            _ => self.region_name.as_deref(),
        };
        match region {
            Some(region) => format!("{}, {region}", self.name),
            None => self.name.clone(),
        }
    }
}

/// The list, or nothing when no file is configured.
#[derive(Debug, Clone, Default)]
pub struct Cities {
    all: Arc<Vec<City>>,
}

/// Rows a search returns.
pub const SEARCH_LIMIT: usize = 6;

impl Cities {
    /// No list: nothing is near anything, and the near line is not shown.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn enabled(&self) -> bool {
        !self.all.is_empty()
    }

    /// Read a `GeoNames` cities file and the `admin1CodesASCII` file that
    /// names its regions. A configured file that does not read is an
    /// error: it should stop the process, not silently drop the line.
    pub fn open(cities: &Path, regions: &Path) -> anyhow::Result<Self> {
        let regions_text = fs::read_to_string(regions)
            .with_context(|| format!("could not read the regions file {}", regions.display()))?;
        let cities_text = fs::read_to_string(cities)
            .with_context(|| format!("could not read the cities file {}", cities.display()))?;
        let list = Self::parse(&cities_text, &regions_text);
        tracing::info!(path = %cities.display(), cities = list.all.len(), "cities loaded");
        Ok(list)
    }

    /// The two files' text, tab-separated as `GeoNames` publishes them.
    pub fn parse(cities: &str, regions: &str) -> Self {
        let region_names: std::collections::HashMap<&str, &str> = regions
            .lines()
            .filter_map(|line| {
                let mut f = line.split('\t');
                Some((f.next()?, f.next()?))
            })
            .collect();
        let mut all: Vec<City> = cities
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() < 15 {
                    return None;
                }
                let id = f[0].parse().ok()?;
                let point = Point::parse(f[4], f[5])?;
                let country = f[8].to_owned();
                let region_code = (!f[10].is_empty()).then(|| f[10].to_owned());
                let region_name = region_code
                    .as_deref()
                    .and_then(|code| region_names.get(format!("{country}.{code}").as_str()))
                    .map(|name| (*name).to_owned());
                Some(City {
                    id,
                    name: f[1].to_owned(),
                    ascii: f[2].to_ascii_lowercase(),
                    point,
                    country,
                    region_code,
                    region_name,
                    population: f[14].parse().unwrap_or(0),
                })
            })
            .collect();
        all.sort_by_key(|c| std::cmp::Reverse(c.population));
        Self { all: Arc::new(all) }
    }

    pub fn get(&self, id: u32) -> Option<&City> {
        self.all.iter().find(|c| c.id == id)
    }

    /// The town closest to a point, by which the located point is named.
    pub fn nearest(&self, point: Point) -> Option<&City> {
        self.all
            .iter()
            .min_by(|a, b| distance_sq(a.point, point).total_cmp(&distance_sq(b.point, point)))
    }

    /// Towns whose name starts with what was typed, the biggest first,
    /// or the closest to `near` first when there is a near point.
    pub fn search(&self, q: &str, near: Option<Point>) -> Vec<&City> {
        let q = q.trim().to_ascii_lowercase();
        if q.chars().count() < 2 {
            return Vec::new();
        }
        let mut found: Vec<&City> = self
            .all
            .iter()
            .filter(|c| c.ascii.starts_with(&q) || c.name.to_lowercase().starts_with(&q))
            .collect();
        if let Some(near) = near {
            found.sort_by(|a, b| distance_sq(a.point, near).total_cmp(&distance_sq(b.point, near)));
        }
        found.truncate(SEARCH_LIMIT);
        found
    }
}

/// Squared distance in degrees, the longitude scaled for the latitude:
/// enough to order towns, and cheap over seventy thousand of them.
fn distance_sq(a: Point, b: Point) -> f64 {
    let scale = a.lat.midpoint(b.lat).to_radians().cos();
    let dlat = a.lat - b.lat;
    let dlon = (a.lon - b.lon) * scale;
    dlat * dlat + dlon * dlon
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Cities {
        Cities::parse(
            include_str!("../tests/fixtures/cities-test.txt"),
            include_str!("../tests/fixtures/admin1-test.txt"),
        )
    }

    fn acton_ma() -> Point {
        Point {
            lat: 42.4851,
            lon: -71.4328,
        }
    }

    #[test]
    fn labels_use_the_code_where_people_do_and_the_name_elsewhere() {
        let cities = fixture();
        assert_eq!(cities.get(4_928_703).unwrap().label(), "Acton, MA");
        assert_eq!(cities.get(5_882_134).unwrap().label(), "Acton, Ontario");
        assert_eq!(cities.get(2_657_697).unwrap().label(), "Acton, England");
        assert_eq!(cities.get(2_643_743).unwrap().label(), "London, England");
        assert_eq!(
            cities.get(2_988_507).unwrap().label(),
            "Paris, Île-de-France"
        );
    }

    #[test]
    fn the_nearest_town_names_a_point() {
        let cities = fixture();
        // A little east of Acton's centroid, still Acton.
        let here = Point {
            lat: 42.48,
            lon: -71.40,
        };
        assert_eq!(cities.nearest(here).unwrap().id, 4_928_703);
        // Flatbush is Brooklyn, not New York City.
        let brooklyn = Point {
            lat: 40.64,
            lon: -73.96,
        };
        assert_eq!(cities.nearest(brooklyn).unwrap().label(), "Brooklyn, NY");
        assert!(Cities::none().nearest(here).is_none());
        assert!(!Cities::none().enabled());
    }

    #[test]
    fn search_matches_a_prefix_and_ranks_by_size_or_by_distance() {
        let cities = fixture();
        // Three Actons: the biggest first when nothing is near...
        let by_size: Vec<String> = cities
            .search("act", None)
            .iter()
            .map(|c| c.label())
            .collect();
        assert_eq!(by_size, ["Acton, England", "Acton, MA", "Acton, Ontario"]);
        // ...and the closest first from Acton, MA.
        let by_distance: Vec<String> = cities
            .search("Acton", Some(acton_ma()))
            .iter()
            .map(|c| c.label())
            .collect();
        assert_eq!(
            by_distance,
            ["Acton, MA", "Acton, Ontario", "Acton, England"]
        );
        assert!(cities.search("a", None).is_empty(), "two characters first");
        assert!(cities.search("zzz", None).is_empty());
        assert_eq!(cities.search("  NEW ", None)[0].id, 5_128_581);
    }
}
