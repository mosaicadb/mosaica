//! A view's `extent` as written, and the frame it gives.
//!
//! One rule for every place a view is declared: the build reads the key from its TOML declaration,
//! and a running service from the body of `PUT /control/views` and `PUT /control/view_groups`.
//! Both hand what they read to [`Extent::compile`] and both turn the result into a frame with
//! [`Extent::stated_frame`] where there are no rows to fit one to.
//!
//! The projection decides the spellings, and the two sets do not overlap. An unprojected view
//! takes `auto` or a box in the coordinates its rows already carry; a projected view takes `auto`
//! or a box in longitude and latitude, which is snapped outward to the smallest aligned square
//! holding it.

use crate::frame::{snap_outward, Snap};
use crate::morton::Bounds;
use crate::projection::Projection;

/// `auto`'s margin when none is written: 1% of the data span on each side.
///
/// Not zero, because the extent is a half-open interval and a point exactly at the maximum would
/// quantise to the clamp.
pub const DEFAULT_AUTO_MARGIN: f64 = 0.01;

/// A view's quantisation frame, as declared.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Extent {
    /// `"auto"`, or `{ auto = true, margin = f }`: the square box around the view's rows, with
    /// `margin` of the data span added on each side.
    Auto { margin: f64 },
    /// `{ min, max }` or `{ x = [a, b], y = [c, d] }`, stated outright.
    Fixed(Bounds),
    /// `{ lon = [a, b], lat = [c, d] }` on a projected view, snapped outward to the enclosing
    /// aligned square.
    LonLat(LonLatBox),
    /// `"auto"` on a projected view: the same snap over the rows' own longitude/latitude box. It
    /// has no margin, the snap being the headroom.
    AutoLonLat,
}

/// A box in longitude and latitude, degrees, WGS84.
///
/// A degenerate box is legal here and not as a [`Bounds`]: `lon = [a, a]` states a meridian, and
/// the snap answers it with the offset cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatBox {
    pub lon_min: f64,
    pub lon_max: f64,
    pub lat_min: f64,
    pub lat_max: f64,
}

impl LonLatBox {
    /// This box's image in the frame the projection produces.
    ///
    /// Every projection in the set is cylindrical, so the image of a longitude/latitude rectangle
    /// is a rectangle and its corners are its bounds. The y axis runs south, so the box's minimum
    /// latitude is its maximum y.
    pub fn project(&self, projection: Projection) -> Bounds {
        let (x_min, y_max) = projection.forward(self.lon_min, self.lat_min);
        let (x_max, y_min) = projection.forward(self.lon_max, self.lat_max);
        Bounds {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }
}

/// A stated longitude/latitude box as a frame: projected, then snapped outward to the smallest
/// aligned square containing it. No data is read.
pub fn snap_lon_lat(projection: Projection, asked: &LonLatBox) -> Snap {
    snap_outward(&asked.project(projection))
}

/// `extent` is the word `"auto"` or a table. A view with no projection takes four spellings, in
/// the units of its own coordinates:
///
/// ```toml
/// extent = "auto"                           # the same as { auto = true, margin = 0.01 }
/// extent = { auto = true, margin = 0.25 }   # a square fitted to the data, plus margin each side
/// extent = { min = -25.0, max = 25.0 }      # one range for both axes
/// extent = { x = [-18, 19], y = [-22, 24] } # a range for each axis
/// ```
///
/// A projected view takes two, in degrees, and widens the box to the smallest aligned square that
/// contains it:
///
/// ```toml
/// extent = "auto"                                     # the data's own longitude and latitude
/// extent = { lon = [-8.6, 1.8], lat = [49.9, 60.9] }  # a box
/// ```
///
/// `auto` reads the view's points to fit the frame, so it is refused where the points source
/// holds no rows. Any other word, an empty table, half a box, a number that is not finite, and a
/// spelling of the other kind of view are refused.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtentKeys {
    /// `true` fits the frame to the points the build reads. `false` is refused, and so is `auto`
    /// beside `min`, `max`, `x` or `y`. A projected view writes `extent = "auto"` instead.
    ///
    /// Default: not set.
    #[serde(default)]
    pub auto: Option<bool>,
    /// Space added to each side of an `auto` frame, as a fraction of the data's span. Finite and
    /// at least 0. Refused without `auto = true`.
    ///
    /// Default: `0.01`.
    #[serde(default)]
    pub margin: Option<f64>,
    /// The low end of both axes, beside `max`.
    ///
    /// Default: not set.
    #[serde(default)]
    pub min: Option<f64>,
    /// The high end of both axes, beside `min`. It must be above `min`.
    ///
    /// Default: not set.
    #[serde(default)]
    pub max: Option<f64>,
    /// The x axis as `[low, high]`, beside `y`, with `high` above `low`.
    ///
    /// Default: not set.
    #[serde(default)]
    pub x: Option<[f64; 2]>,
    /// The y axis as `[low, high]`, beside `x`, with `high` above `low`.
    ///
    /// Default: not set.
    #[serde(default)]
    pub y: Option<[f64; 2]>,
    /// A projected view's longitudes as `[west, east]`, in degrees within ±180, beside `lat`. A
    /// box crossing the antimeridian, with `west` above `east`, is refused.
    ///
    /// Default: not set.
    #[serde(default)]
    pub lon: Option<[f64; 2]>,
    /// A projected view's latitudes as `[south, north]`, in degrees within ±90, beside `lon`,
    /// with `north` not below `south`.
    ///
    /// Default: not set.
    #[serde(default)]
    pub lat: Option<[f64; 2]>,
}

/// An `extent` as a declaration or a request body wrote it, read from either.
#[derive(Debug, Clone, PartialEq)]
pub enum WrittenExtent {
    /// A string.
    Word(String),
    /// A table, its keys read.
    Keys(ExtentKeys),
    /// Anything else: a number, a list, a boolean.
    Neither,
}

impl<'de> serde::Deserialize<'de> for WrittenExtent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
        use serde::Deserialize as _;

        struct Written;

        impl<'de> Visitor<'de> for Written {
            type Value = WrittenExtent;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("the word `auto` or a table")
            }

            fn visit_str<E: serde::de::Error>(self, word: &str) -> Result<WrittenExtent, E> {
                Ok(WrittenExtent::Word(word.to_string()))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<WrittenExtent, A::Error> {
                ExtentKeys::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(WrittenExtent::Keys)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<WrittenExtent, A::Error> {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(WrittenExtent::Neither)
            }

            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<WrittenExtent, E> {
                Ok(WrittenExtent::Neither)
            }

            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<WrittenExtent, E> {
                Ok(WrittenExtent::Neither)
            }

            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<WrittenExtent, E> {
                Ok(WrittenExtent::Neither)
            }

            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<WrittenExtent, E> {
                Ok(WrittenExtent::Neither)
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<WrittenExtent, E> {
                Ok(WrittenExtent::Neither)
            }
        }

        deserializer.deserialize_any(Written)
    }
}

/// A projected view's two spellings, appended to every `extent` refusal it earns.
const LON_LAT_SPELLINGS: &str = "\n  \
     extent = \"auto\"                                         # the data's own box, snapped\n  \
     extent = { lon = [-8.6, 1.8], lat = [49.9, 60.9] }      # stated in degrees, snapped outward";

/// An unprojected view's four spellings, appended to every `extent` refusal it earns.
const EXTENT_SPELLINGS: &str = "\n  \
     extent = \"auto\"                          # the square box around the data, small margin\n  \
     extent = { auto = true, margin = 0.25 }  # a quarter of the data span as headroom each side\n  \
     extent = { min = -25.0, max = 25.0 }     # one range, both axes — preserves aspect ratio\n  \
     extent = { x = [-18, 19], y = [-22, 24] }  # per axis, where stretching is meant";

fn spellings(projection: Projection) -> &'static str {
    match projection {
        Projection::None => EXTENT_SPELLINGS,
        _ => LON_LAT_SPELLINGS,
    }
}

/// `auto` where there are no rows to fit a frame to: what is wrong, and the boxes to state
/// instead. `subject` names the view or group, as `view 'world'`.
pub fn auto_without_rows(subject: &str, projection: Projection) -> String {
    format!(
        "{subject}: `extent` is `auto`, which fits the frame to the view's rows, and there are no \
         rows to fit it to. A view that starts empty, or whose rows --limit excluded, needs its \
         frame stated:{}",
        spellings(projection)
    )
}

impl Extent {
    /// Compile `written`, the `extent` of the view or group `object` names, under `projection`.
    /// `None` is the key left out, which is refused: the frame has no default.
    pub fn compile(
        object: &str,
        projection: Projection,
        written: Option<WrittenExtent>,
    ) -> Result<Extent, String> {
        let projected = projection != Projection::None;
        let spellings = spellings(projection);
        let keys = match written {
            None => {
                return Err(format!(
                    "{object}: `extent` is required and has no default. It is the frame every \
                     stored position is quantised across, and quantisation clamps, so a guessed \
                     frame is a bundle that is well-formed with the geometry wrong. This view's \
                     spellings:{spellings}"
                ))
            }
            Some(WrittenExtent::Word(word)) if word == "auto" => {
                return Ok(match projected {
                    true => Extent::AutoLonLat,
                    false => Extent::Auto {
                        margin: DEFAULT_AUTO_MARGIN,
                    },
                })
            }
            Some(WrittenExtent::Word(word)) => {
                return Err(format!(
                    "{object}: `extent = \"{word}\"` is not a value this key takes. The only word \
                     it takes is `auto`; every other spelling is a table:{spellings}"
                ))
            }
            Some(WrittenExtent::Neither) => {
                return Err(format!(
                    "{object}: `extent` is neither the word `auto` nor a table:{spellings}"
                ))
            }
            Some(WrittenExtent::Keys(keys)) => keys,
        };
        if projected {
            return compile_lon_lat(object, projection, &keys);
        }
        if keys.lon.is_some() || keys.lat.is_some() {
            return Err(format!(
                "{object}: `extent` is written in longitude and latitude, and this view declares \
                 no projection, so the two numbers would be quantised as though they were the \
                 rows' own units. Declare `projection = \"web_mercator\"` or \
                 `projection = \"equirectangular\"` if these are places on the Earth; otherwise \
                 state the frame in the coordinates the rows carry:{EXTENT_SPELLINGS}"
            ));
        }
        let stated = keys.min.is_some() || keys.max.is_some() || keys.x.is_some() || keys.y.is_some();
        if let Some(auto) = keys.auto {
            if !auto {
                return Err(format!(
                    "{object}: `extent = {{ auto = false }}` says what the frame is not. Write \
                     the frame:{EXTENT_SPELLINGS}"
                ));
            }
            if stated {
                return Err(format!(
                    "{object}: `extent` declares `auto` and a stated box together. `auto` fits \
                     the box to the view's rows; `min`/`max` and `x`/`y` state it outright. One \
                     or the other:{EXTENT_SPELLINGS}"
                ));
            }
            let margin = match keys.margin {
                None => DEFAULT_AUTO_MARGIN,
                Some(margin) if !margin.is_finite() || margin < 0.0 => {
                    return Err(format!(
                        "{object}: `extent.margin = {margin}` is not a fraction of the data span. \
                         It is headroom added on each side, so it is finite and at least 0: a \
                         negative margin would shrink the box inside the data and clamp the \
                         points it excluded"
                    ))
                }
                Some(margin) => margin,
            };
            return Ok(Extent::Auto { margin });
        }
        if keys.margin.is_some() {
            return Err(format!(
                "{object}: `extent.margin` without `auto = true`. A margin is headroom around a \
                 box fitted to data; a box stated outright already includes whatever headroom its \
                 author wanted:{EXTENT_SPELLINGS}"
            ));
        }
        let bounds = match (keys.min, keys.max, keys.x, keys.y) {
            (Some(min), Some(max), None, None) => Bounds {
                x_min: min,
                x_max: max,
                y_min: min,
                y_max: max,
            },
            (None, None, Some([x_min, x_max]), Some([y_min, y_max])) => Bounds {
                x_min,
                x_max,
                y_min,
                y_max,
            },
            (None, None, None, None) => {
                return Err(format!(
                    "{object}: `extent` is an empty table, so it declares no frame at \
                     all:{EXTENT_SPELLINGS}"
                ))
            }
            (min, max, x, y) => {
                let named = [
                    min.map(|_| "min"),
                    max.map(|_| "max"),
                    x.map(|_| "x"),
                    y.map(|_| "y"),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
                return Err(format!(
                    "{object}: `extent` names {named}, which is half a frame. `min` and `max` \
                     give one range to both axes and preserve the aspect ratio; `x` and `y` give \
                     a range each, where stretching is meant. Neither half stands \
                     alone:{EXTENT_SPELLINGS}"
                ));
            }
        };
        bounds
            .validate()
            .map_err(|detail| format!("{object}: `extent`: {detail}"))?;
        Ok(Extent::Fixed(bounds))
    }

    /// The frame this extent states, with no rows to fit one to: a stated box as written, and a
    /// degree box snapped. `auto` has nothing to fit and is refused by [`auto_without_rows`].
    pub fn stated_frame(&self, subject: &str, projection: Projection) -> Result<Bounds, String> {
        match self {
            Extent::Fixed(bounds) => Ok(*bounds),
            Extent::LonLat(asked) => Ok(snap_lon_lat(projection, asked).square.bounds()),
            Extent::Auto { .. } | Extent::AutoLonLat => {
                Err(auto_without_rows(subject, projection))
            }
        }
    }
}

/// A projected view's `extent`: `auto`, or the box in longitude and latitude. Every other
/// spelling is refused here, naming this one.
fn compile_lon_lat(object: &str, projection: Projection, keys: &ExtentKeys) -> Result<Extent, String> {
    let name = projection.name();
    if keys.auto.is_some() || keys.margin.is_some() {
        return Err(format!(
            "{object}: `extent` declares `auto` as a table, and the view is projected ({name}). A \
             projected frame's headroom is the outward snap to the enclosing aligned square, not a \
             fraction of the data span. Write `extent = \"auto\"`, which fits the rows' own \
             longitude/latitude box and snaps it:{LON_LAT_SPELLINGS}"
        ));
    }
    if keys.min.is_some() || keys.max.is_some() || keys.x.is_some() || keys.y.is_some() {
        let named = [
            keys.min.map(|_| "min"),
            keys.max.map(|_| "max"),
            keys.x.map(|_| "x"),
            keys.y.map(|_| "y"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
        return Err(format!(
            "{object}: `extent` names {named}, and this view is projected ({name}). Those \
             spellings state a frame in the space the projection produces, which nobody should \
             work out by hand. A projected view's frame is written in degrees:{LON_LAT_SPELLINGS}"
        ));
    }
    let (Some(lon), Some(lat)) = (keys.lon, keys.lat) else {
        let half = match (keys.lon, keys.lat) {
            (Some(_), None) => "`lon` without `lat`",
            (None, Some(_)) => "`lat` without `lon`",
            _ => "no frame at all",
        };
        return Err(format!(
            "{object}: `extent` is {half}. A projected view's frame is a box in longitude and \
             latitude and neither half stands alone:{LON_LAT_SPELLINGS}"
        ));
    };
    for (axis, limit, pair) in [("lon", 180.0, lon), ("lat", 90.0, lat)] {
        for v in pair {
            if !v.is_finite() || f64::abs(v) > limit {
                return Err(format!(
                    "{object}: `extent.{axis}` names {v}, which is not a {}. The input coordinate \
                     system is WGS84 degrees, longitude within ±180 and latitude within ±90. \
                     Convert the box to WGS84, or declare `projection = \"none\"` if this view's \
                     space is not the Earth",
                    if axis == "lon" { "longitude" } else { "latitude" }
                ));
            }
        }
    }
    if lon[0] > lon[1] {
        return Err(format!(
            "{object}: `extent.lon = [{}, {}]` runs west from its own maximum. A frame is one \
             aligned square and does not wrap, so a box crossing the antimeridian cannot be \
             honoured. Write the wider box that does not cross: `lon = [{}, {}]`",
            lon[0], lon[1], lon[1], lon[0]
        ));
    }
    if lat[0] > lat[1] {
        return Err(format!(
            "{object}: `extent.lat = [{}, {}]` runs south from its own maximum, so it names no \
             box. Latitude does not wrap; write `lat = [{}, {}]`",
            lat[0], lat[1], lat[1], lat[0]
        ));
    }
    Ok(Extent::LonLat(LonLatBox {
        lon_min: lon[0],
        lon_max: lon[1],
        lat_min: lat[0],
        lat_max: lat[1],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(f: impl FnOnce(&mut ExtentKeys)) -> Option<WrittenExtent> {
        let mut keys = ExtentKeys::default();
        f(&mut keys);
        Some(WrittenExtent::Keys(keys))
    }

    #[test]
    fn a_stated_box_is_its_own_frame() {
        let extent = Extent::compile(
            "view 'v'",
            Projection::None,
            keys(|k| {
                k.min = Some(-1.0);
                k.max = Some(1.0);
            }),
        )
        .unwrap();
        let frame = extent.stated_frame("view 'v'", Projection::None).unwrap();
        assert_eq!(
            frame,
            Bounds {
                x_min: -1.0,
                x_max: 1.0,
                y_min: -1.0,
                y_max: 1.0
            }
        );
    }

    #[test]
    fn a_degree_box_is_snapped_to_an_aligned_square() {
        let asked = LonLatBox {
            lon_min: -8.6,
            lon_max: 1.8,
            lat_min: 49.9,
            lat_max: 60.9,
        };
        let extent = Extent::compile(
            "view 'v'",
            Projection::WebMercator,
            keys(|k| {
                k.lon = Some([asked.lon_min, asked.lon_max]);
                k.lat = Some([asked.lat_min, asked.lat_max]);
            }),
        )
        .unwrap();
        assert_eq!(
            extent.stated_frame("view 'v'", Projection::WebMercator).unwrap(),
            snap_lon_lat(Projection::WebMercator, &asked).square.bounds()
        );
    }

    #[test]
    fn auto_has_no_frame_without_rows() {
        for projection in [Projection::None, Projection::WebMercator] {
            let extent =
                Extent::compile("view 'v'", projection, Some(WrittenExtent::Word("auto".into())))
                    .unwrap();
            assert!(extent.stated_frame("view 'v'", projection).is_err());
        }
    }

    #[test]
    fn each_kind_of_view_refuses_the_other_kinds_spelling() {
        let degrees = || {
            keys(|k| {
                k.lon = Some([0.0, 1.0]);
                k.lat = Some([0.0, 1.0]);
            })
        };
        let frame = || {
            keys(|k| {
                k.x = Some([0.0, 1.0]);
                k.y = Some([0.0, 1.0]);
            })
        };
        assert!(Extent::compile("v", Projection::None, degrees()).is_err());
        assert!(Extent::compile("v", Projection::WebMercator, frame()).is_err());
        assert!(Extent::compile("v", Projection::None, None).is_err());
        assert!(Extent::compile("v", Projection::None, Some(WrittenExtent::Neither)).is_err());
    }
}
