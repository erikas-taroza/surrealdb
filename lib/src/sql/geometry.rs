#![allow(clippy::derived_hash_with_manual_eq)]

use crate::sql::array::Array;
use crate::sql::comment::mightbespace;
use crate::sql::common::{
	closebraces, closebracket, closeparentheses, commas, openbraces, openbracket, openparentheses,
};
use crate::sql::error::IResult;
use crate::sql::fmt::Fmt;
use crate::sql::value::Value;
use geo::algorithm::contains::Contains;
use geo::algorithm::intersects::Intersects;
use geo::{Coord, LineString, Point, Polygon};
use geo::{MultiLineString, MultiPoint, MultiPolygon};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::opt;
use nom::number::complete::double;
use nom::sequence::preceded;
use nom::sequence::{delimited, terminated};
use revision::revisioned;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::iter::{once, FromIterator};
use std::{fmt, hash};

use super::util::{delimited_list0, delimited_list1};

pub(crate) const TOKEN: &str = "$surrealdb::private::sql::Geometry";

const SINGLE: char = '\'';
const DOUBLE: char = '\"';

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "$surrealdb::private::sql::Geometry")]
#[revisioned(revision = 1)]
pub enum Geometry {
	Point(Point<f64>),
	Line(LineString<f64>),
	Polygon(Polygon<f64>),
	MultiPoint(MultiPoint<f64>),
	MultiLine(MultiLineString<f64>),
	MultiPolygon(MultiPolygon<f64>),
	Collection(Vec<Geometry>),
	// Add new variants here
}

impl Geometry {
	/// Check if this is a Point
	pub fn is_point(&self) -> bool {
		matches!(self, Self::Point(_))
	}
	/// Check if this is a Line
	pub fn is_line(&self) -> bool {
		matches!(self, Self::Line(_))
	}
	/// Check if this is a Polygon
	pub fn is_polygon(&self) -> bool {
		matches!(self, Self::Polygon(_))
	}
	/// Check if this is a MultiPoint
	pub fn is_multipoint(&self) -> bool {
		matches!(self, Self::MultiPoint(_))
	}
	/// Check if this is a MultiLine
	pub fn is_multiline(&self) -> bool {
		matches!(self, Self::MultiLine(_))
	}
	/// Check if this is a MultiPolygon
	pub fn is_multipolygon(&self) -> bool {
		matches!(self, Self::MultiPolygon(_))
	}
	/// Check if this is not a Collection
	pub fn is_geometry(&self) -> bool {
		!matches!(self, Self::Collection(_))
	}
	/// Check if this is a Collection
	pub fn is_collection(&self) -> bool {
		matches!(self, Self::Collection(_))
	}
	/// Get the type of this Geometry as text
	pub fn as_type(&self) -> &'static str {
		match self {
			Self::Point(_) => "Point",
			Self::Line(_) => "LineString",
			Self::Polygon(_) => "Polygon",
			Self::MultiPoint(_) => "MultiPoint",
			Self::MultiLine(_) => "MultiLineString",
			Self::MultiPolygon(_) => "MultiPolygon",
			Self::Collection(_) => "GeometryCollection",
		}
	}
	/// Get the raw coordinates of this Geometry as an Array
	pub fn as_coordinates(&self) -> Value {
		fn point(v: &Point) -> Value {
			Array::from(vec![v.x(), v.y()]).into()
		}

		fn line(v: &LineString) -> Value {
			v.points().map(|v| point(&v)).collect::<Vec<Value>>().into()
		}

		fn polygon(v: &Polygon) -> Value {
			once(v.exterior()).chain(v.interiors()).map(line).collect::<Vec<Value>>().into()
		}

		fn multipoint(v: &MultiPoint) -> Value {
			v.iter().map(point).collect::<Vec<Value>>().into()
		}

		fn multiline(v: &MultiLineString) -> Value {
			v.iter().map(line).collect::<Vec<Value>>().into()
		}

		fn multipolygon(v: &MultiPolygon) -> Value {
			v.iter().map(polygon).collect::<Vec<Value>>().into()
		}

		fn collection(v: &[Geometry]) -> Value {
			v.iter().map(Geometry::as_coordinates).collect::<Vec<Value>>().into()
		}

		match self {
			Self::Point(v) => point(v),
			Self::Line(v) => line(v),
			Self::Polygon(v) => polygon(v),
			Self::MultiPoint(v) => multipoint(v),
			Self::MultiLine(v) => multiline(v),
			Self::MultiPolygon(v) => multipolygon(v),
			Self::Collection(v) => collection(v),
		}
	}
}

impl PartialOrd for Geometry {
	#[rustfmt::skip]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		fn coord(v: &Coord) -> (f64, f64) {
			v.x_y()
		}

		fn point(v: &Point) -> (f64, f64) {
			coord(&v.0)
		}

		fn line(v: &LineString) -> impl Iterator<Item = (f64, f64)> + '_ {
			v.into_iter().map(coord)
		}

		fn polygon(v: &Polygon) -> impl Iterator<Item = (f64, f64)> + '_ {
			v.interiors().iter().chain(once(v.exterior())).flat_map(line)
		}

		fn multipoint(v: &MultiPoint) -> impl Iterator<Item = (f64, f64)> + '_ {
			v.iter().map(point)
		}

		fn multiline(v: &MultiLineString) -> impl Iterator<Item = (f64, f64)> + '_ {
			v.iter().flat_map(line)
		}

		fn multipolygon(v: &MultiPolygon) -> impl Iterator<Item = (f64, f64)> + '_ {
			v.iter().flat_map(polygon)
		}

		match (self, other) {
			//
			(Self::Point(_), Self::Line(_)) => Some(Ordering::Less),
			(Self::Point(_), Self::Polygon(_)) => Some(Ordering::Less),
			(Self::Point(_), Self::MultiPoint(_)) => Some(Ordering::Less),
			(Self::Point(_), Self::MultiLine(_)) => Some(Ordering::Less),
			(Self::Point(_), Self::MultiPolygon(_)) => Some(Ordering::Less),
			(Self::Point(_), Self::Collection(_)) => Some(Ordering::Less),
			//
			(Self::Line(_), Self::Point(_)) => Some(Ordering::Greater),
			(Self::Line(_), Self::Polygon(_)) => Some(Ordering::Less),
			(Self::Line(_), Self::MultiPoint(_)) => Some(Ordering::Less),
			(Self::Line(_), Self::MultiLine(_)) => Some(Ordering::Less),
			(Self::Line(_), Self::MultiPolygon(_)) => Some(Ordering::Less),
			(Self::Line(_), Self::Collection(_)) => Some(Ordering::Less),
			//
			(Self::Polygon(_), Self::Point(_)) => Some(Ordering::Greater),
			(Self::Polygon(_), Self::Line(_)) => Some(Ordering::Greater),
			(Self::Polygon(_), Self::MultiPoint(_)) => Some(Ordering::Less),
			(Self::Polygon(_), Self::MultiLine(_)) => Some(Ordering::Less),
			(Self::Polygon(_), Self::MultiPolygon(_)) => Some(Ordering::Less),
			(Self::Polygon(_), Self::Collection(_)) => Some(Ordering::Less),
			//
			(Self::MultiPoint(_), Self::Point(_)) => Some(Ordering::Greater),
			(Self::MultiPoint(_), Self::Line(_)) => Some(Ordering::Greater),
			(Self::MultiPoint(_), Self::Polygon(_)) => Some(Ordering::Greater),
			(Self::MultiPoint(_), Self::MultiLine(_)) => Some(Ordering::Less),
			(Self::MultiPoint(_), Self::MultiPolygon(_)) => Some(Ordering::Less),
			(Self::MultiPoint(_), Self::Collection(_)) => Some(Ordering::Less),
			//
			(Self::MultiLine(_), Self::Point(_)) => Some(Ordering::Greater),
			(Self::MultiLine(_), Self::Line(_)) => Some(Ordering::Greater),
			(Self::MultiLine(_), Self::Polygon(_)) => Some(Ordering::Greater),
			(Self::MultiLine(_), Self::MultiPoint(_)) => Some(Ordering::Greater),
			(Self::MultiLine(_), Self::MultiPolygon(_)) => Some(Ordering::Less),
			(Self::MultiLine(_), Self::Collection(_)) => Some(Ordering::Less),
			//
			(Self::MultiPolygon(_), Self::Point(_)) => Some(Ordering::Greater),
			(Self::MultiPolygon(_), Self::Line(_)) => Some(Ordering::Greater),
			(Self::MultiPolygon(_), Self::Polygon(_)) => Some(Ordering::Greater),
			(Self::MultiPolygon(_), Self::MultiPoint(_)) => Some(Ordering::Greater),
			(Self::MultiPolygon(_), Self::MultiLine(_)) => Some(Ordering::Greater),
			(Self::MultiPolygon(_), Self::Collection(_)) => Some(Ordering::Less),
			//
			(Self::Collection(_), Self::Point(_)) => Some(Ordering::Greater),
			(Self::Collection(_), Self::Line(_)) => Some(Ordering::Greater),
			(Self::Collection(_), Self::Polygon(_)) => Some(Ordering::Greater),
			(Self::Collection(_), Self::MultiPoint(_)) => Some(Ordering::Greater),
			(Self::Collection(_), Self::MultiLine(_)) => Some(Ordering::Greater),
			(Self::Collection(_), Self::MultiPolygon(_)) => Some(Ordering::Greater),
			//
			(Self::Point(a), Self::Point(b)) => point(a).partial_cmp(&point(b)),
			(Self::Line(a), Self::Line(b)) => line(a).partial_cmp(line(b)),
			(Self::Polygon(a), Self::Polygon(b)) => polygon(a).partial_cmp(polygon(b)),
			(Self::MultiPoint(a), Self::MultiPoint(b)) => multipoint(a).partial_cmp(multipoint(b)),
			(Self::MultiLine(a), Self::MultiLine(b)) => multiline(a).partial_cmp(multiline(b)),
			(Self::MultiPolygon(a), Self::MultiPolygon(b)) => multipolygon(a).partial_cmp(multipolygon(b)),
			(Self::Collection(a), Self::Collection(b)) => a.partial_cmp(b),
		}
	}
}

impl From<(f64, f64)> for Geometry {
	fn from(v: (f64, f64)) -> Self {
		Self::Point(v.into())
	}
}

impl From<[f64; 2]> for Geometry {
	fn from(v: [f64; 2]) -> Self {
		Self::Point(v.into())
	}
}

impl From<Point<f64>> for Geometry {
	fn from(v: Point<f64>) -> Self {
		Self::Point(v)
	}
}

impl From<LineString<f64>> for Geometry {
	fn from(v: LineString<f64>) -> Self {
		Self::Line(v)
	}
}

impl From<Polygon<f64>> for Geometry {
	fn from(v: Polygon<f64>) -> Self {
		Self::Polygon(v)
	}
}

impl From<MultiPoint<f64>> for Geometry {
	fn from(v: MultiPoint<f64>) -> Self {
		Self::MultiPoint(v)
	}
}

impl From<MultiLineString<f64>> for Geometry {
	fn from(v: MultiLineString<f64>) -> Self {
		Self::MultiLine(v)
	}
}

impl From<MultiPolygon<f64>> for Geometry {
	fn from(v: MultiPolygon<f64>) -> Self {
		Self::MultiPolygon(v)
	}
}

impl From<Vec<Geometry>> for Geometry {
	fn from(v: Vec<Geometry>) -> Self {
		Self::Collection(v)
	}
}

impl From<Vec<Point<f64>>> for Geometry {
	fn from(v: Vec<Point<f64>>) -> Self {
		Self::MultiPoint(MultiPoint(v))
	}
}

impl From<Vec<LineString<f64>>> for Geometry {
	fn from(v: Vec<LineString<f64>>) -> Self {
		Self::MultiLine(MultiLineString(v))
	}
}

impl From<Vec<Polygon<f64>>> for Geometry {
	fn from(v: Vec<Polygon<f64>>) -> Self {
		Self::MultiPolygon(MultiPolygon(v))
	}
}

impl From<Geometry> for geo::Geometry<f64> {
	fn from(v: Geometry) -> Self {
		match v {
			Geometry::Point(v) => v.into(),
			Geometry::Line(v) => v.into(),
			Geometry::Polygon(v) => v.into(),
			Geometry::MultiPoint(v) => v.into(),
			Geometry::MultiLine(v) => v.into(),
			Geometry::MultiPolygon(v) => v.into(),
			Geometry::Collection(v) => v.into_iter().collect::<geo::Geometry<f64>>(),
		}
	}
}

impl FromIterator<Geometry> for geo::Geometry<f64> {
	fn from_iter<I: IntoIterator<Item = Geometry>>(iter: I) -> Self {
		let mut c: Vec<geo::Geometry<f64>> = vec![];
		for i in iter {
			c.push(i.into())
		}
		geo::Geometry::GeometryCollection(geo::GeometryCollection(c))
	}
}

impl Geometry {
	// -----------------------------------
	// Value operations
	// -----------------------------------

	pub fn contains(&self, other: &Self) -> bool {
		match self {
			Self::Point(v) => match other {
				Self::Point(w) => v.contains(w),
				Self::MultiPoint(w) => w.iter().all(|x| v.contains(x)),
				Self::Collection(w) => w.iter().all(|x| self.contains(x)),
				_ => false,
			},
			Self::Line(v) => match other {
				Self::Point(w) => v.contains(w),
				Self::Line(w) => v.contains(w),
				Self::MultiLine(w) => w.iter().all(|x| w.contains(x)),
				Self::Collection(w) => w.iter().all(|x| self.contains(x)),
				_ => false,
			},
			Self::Polygon(v) => match other {
				Self::Point(w) => v.contains(w),
				Self::Line(w) => v.contains(w),
				Self::Polygon(w) => v.contains(w),
				Self::MultiPolygon(w) => w.iter().all(|x| w.contains(x)),
				Self::Collection(w) => w.iter().all(|x| self.contains(x)),
				_ => false,
			},
			Self::MultiPoint(v) => match other {
				Self::Point(w) => v.contains(w),
				Self::MultiPoint(w) => w.iter().all(|x| w.contains(x)),
				Self::Collection(w) => w.iter().all(|x| self.contains(x)),
				_ => false,
			},
			Self::MultiLine(v) => match other {
				Self::Point(w) => v.contains(w),
				Self::Line(w) => v.contains(w),
				Self::MultiLine(w) => w.iter().all(|x| w.contains(x)),
				Self::Collection(w) => w.iter().all(|x| self.contains(x)),
				_ => false,
			},
			Self::MultiPolygon(v) => match other {
				Self::Point(w) => v.contains(w),
				Self::Line(w) => v.contains(w),
				Self::Polygon(w) => v.contains(w),
				Self::MultiPoint(w) => v.contains(w),
				Self::MultiLine(w) => v.contains(w),
				Self::MultiPolygon(w) => v.contains(w),
				Self::Collection(w) => w.iter().all(|x| self.contains(x)),
			},
			Self::Collection(v) => v.iter().all(|x| x.contains(other)),
		}
	}

	pub fn intersects(&self, other: &Self) -> bool {
		match self {
			Self::Point(v) => match other {
				Self::Point(w) => v.intersects(w),
				Self::Line(w) => v.intersects(w),
				Self::Polygon(w) => v.intersects(w),
				Self::MultiPoint(w) => v.intersects(w),
				Self::MultiLine(w) => w.iter().any(|x| v.intersects(x)),
				Self::MultiPolygon(w) => v.intersects(w),
				Self::Collection(w) => w.iter().all(|x| self.intersects(x)),
			},
			Self::Line(v) => match other {
				Self::Point(w) => v.intersects(w),
				Self::Line(w) => v.intersects(w),
				Self::Polygon(w) => v.intersects(w),
				Self::MultiPoint(w) => v.intersects(w),
				Self::MultiLine(w) => w.iter().any(|x| v.intersects(x)),
				Self::MultiPolygon(w) => v.intersects(w),
				Self::Collection(w) => w.iter().all(|x| self.intersects(x)),
			},
			Self::Polygon(v) => match other {
				Self::Point(w) => v.intersects(w),
				Self::Line(w) => v.intersects(w),
				Self::Polygon(w) => v.intersects(w),
				Self::MultiPoint(w) => v.intersects(w),
				Self::MultiLine(w) => v.intersects(w),
				Self::MultiPolygon(w) => v.intersects(w),
				Self::Collection(w) => w.iter().all(|x| self.intersects(x)),
			},
			Self::MultiPoint(v) => match other {
				Self::Point(w) => v.intersects(w),
				Self::Line(w) => v.intersects(w),
				Self::Polygon(w) => v.intersects(w),
				Self::MultiPoint(w) => v.intersects(w),
				Self::MultiLine(w) => w.iter().any(|x| v.intersects(x)),
				Self::MultiPolygon(w) => v.intersects(w),
				Self::Collection(w) => w.iter().all(|x| self.intersects(x)),
			},
			Self::MultiLine(v) => match other {
				Self::Point(w) => v.intersects(w),
				Self::Line(w) => v.intersects(w),
				Self::Polygon(w) => v.intersects(w),
				Self::MultiPoint(w) => v.intersects(w),
				Self::MultiLine(w) => w.iter().any(|x| v.intersects(x)),
				Self::MultiPolygon(w) => v.intersects(w),
				Self::Collection(w) => w.iter().all(|x| self.intersects(x)),
			},
			Self::MultiPolygon(v) => match other {
				Self::Point(w) => v.intersects(w),
				Self::Line(w) => v.intersects(w),
				Self::Polygon(w) => v.intersects(w),
				Self::MultiPoint(w) => v.intersects(w),
				Self::MultiLine(w) => v.intersects(w),
				Self::MultiPolygon(w) => v.intersects(w),
				Self::Collection(w) => w.iter().all(|x| self.intersects(x)),
			},
			Self::Collection(v) => v.iter().all(|x| x.intersects(other)),
		}
	}
}

impl fmt::Display for Geometry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Point(v) => {
				write!(f, "({}, {})", v.x(), v.y())
			}
			Self::Line(v) => write!(
				f,
				"{{ type: 'LineString', coordinates: [{}] }}",
				Fmt::comma_separated(v.points().map(|v| Fmt::new(v, |v, f| write!(
					f,
					"[{}, {}]",
					v.x(),
					v.y()
				))))
			),
			Self::Polygon(v) => write!(
				f,
				"{{ type: 'Polygon', coordinates: [[{}]{}] }}",
				Fmt::comma_separated(v.exterior().points().map(|v| Fmt::new(v, |v, f| write!(
					f,
					"[{}, {}]",
					v.x(),
					v.y()
				)))),
				Fmt::new(v.interiors(), |interiors, f| {
					match interiors.len() {
						0 => Ok(()),
						_ => write!(
							f,
							", [{}]",
							Fmt::comma_separated(interiors.iter().map(|i| Fmt::new(i, |i, f| {
								write!(
									f,
									"[{}]",
									Fmt::comma_separated(i.points().map(|v| Fmt::new(
										v,
										|v, f| write!(f, "[{}, {}]", v.x(), v.y())
									)))
								)
							})))
						),
					}
				})
			),
			Self::MultiPoint(v) => {
				write!(
					f,
					"{{ type: 'MultiPoint', coordinates: [{}] }}",
					Fmt::comma_separated(v.iter().map(|v| Fmt::new(v, |v, f| write!(
						f,
						"[{}, {}]",
						v.x(),
						v.y()
					))))
				)
			}
			Self::MultiLine(v) => write!(
				f,
				"{{ type: 'MultiLineString', coordinates: [{}] }}",
				Fmt::comma_separated(v.iter().map(|v| Fmt::new(v, |v, f| write!(
					f,
					"[{}]",
					Fmt::comma_separated(v.points().map(|v| Fmt::new(v, |v, f| write!(
						f,
						"[{}, {}]",
						v.x(),
						v.y()
					))))
				))))
			),
			Self::MultiPolygon(v) => write!(
				f,
				"{{ type: 'MultiPolygon', coordinates: [{}] }}",
				Fmt::comma_separated(v.iter().map(|v| Fmt::new(v, |v, f| {
					write!(
						f,
						"[[{}]{}]",
						Fmt::comma_separated(
							v.exterior().points().map(|v| Fmt::new(v, |v, f| write!(
								f,
								"[{}, {}]",
								v.x(),
								v.y()
							)))
						),
						Fmt::new(v.interiors(), |interiors, f| {
							match interiors.len() {
								0 => Ok(()),
								_ => write!(
									f,
									", [{}]",
									Fmt::comma_separated(interiors.iter().map(|i| Fmt::new(
										i,
										|i, f| {
											write!(
												f,
												"[{}]",
												Fmt::comma_separated(i.points().map(|v| Fmt::new(
													v,
													|v, f| write!(f, "[{}, {}]", v.x(), v.y())
												)))
											)
										}
									)))
								),
							}
						})
					)
				}))),
			),
			Self::Collection(v) => {
				write!(
					f,
					"{{ type: 'GeometryCollection', geometries: [{}] }}",
					Fmt::comma_separated(v)
				)
			}
		}
	}
}

impl hash::Hash for Geometry {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		match self {
			Geometry::Point(p) => {
				"Point".hash(state);
				p.x().to_bits().hash(state);
				p.y().to_bits().hash(state);
			}
			Geometry::Line(l) => {
				"Line".hash(state);
				l.points().for_each(|v| {
					v.x().to_bits().hash(state);
					v.y().to_bits().hash(state);
				});
			}
			Geometry::Polygon(p) => {
				"Polygon".hash(state);
				p.exterior().points().for_each(|ext| {
					ext.x().to_bits().hash(state);
					ext.y().to_bits().hash(state);
				});
				p.interiors().iter().for_each(|int| {
					int.points().for_each(|v| {
						v.x().to_bits().hash(state);
						v.y().to_bits().hash(state);
					});
				});
			}
			Geometry::MultiPoint(v) => {
				"MultiPoint".hash(state);
				v.0.iter().for_each(|v| {
					v.x().to_bits().hash(state);
					v.y().to_bits().hash(state);
				});
			}
			Geometry::MultiLine(ml) => {
				"MultiLine".hash(state);
				ml.0.iter().for_each(|ls| {
					ls.points().for_each(|p| {
						p.x().to_bits().hash(state);
						p.y().to_bits().hash(state);
					});
				});
			}
			Geometry::MultiPolygon(mp) => {
				"MultiPolygon".hash(state);
				mp.0.iter().for_each(|p| {
					p.exterior().points().for_each(|ext| {
						ext.x().to_bits().hash(state);
						ext.y().to_bits().hash(state);
					});
					p.interiors().iter().for_each(|int| {
						int.points().for_each(|v| {
							v.x().to_bits().hash(state);
							v.y().to_bits().hash(state);
						});
					});
				});
			}
			Geometry::Collection(v) => {
				"GeometryCollection".hash(state);
				v.iter().for_each(|v| v.hash(state));
			}
		}
	}
}

pub fn geometry(i: &str) -> IResult<&str, Geometry> {
	let _diving = crate::sql::parser::depth::dive()?;
	alt((simple, normal))(i)
}

fn simple(i: &str) -> IResult<&str, Geometry> {
	let (i, _) = openparentheses(i)?;
	let (i, x) = double(i)?;
	let (i, _) = commas(i)?;
	let (i, y) = double(i)?;
	let (i, _) = closeparentheses(i)?;
	Ok((i, Geometry::Point((x, y).into())))
}

fn normal(i: &str) -> IResult<&str, Geometry> {
	let (i, _) = openbraces(i)?;
	let (i, v) = alt((point, line, polygon, multipoint, multiline, multipolygon, collection))(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, _) = opt(char(','))(i)?;
	let (i, _) = closebraces(i)?;
	Ok((i, v))
}

fn point(i: &str) -> IResult<&str, Geometry> {
	let (i, v) = alt((
		|i| {
			let (i, _) = preceded(key_type, point_type)(i)?;
			let (i, _) = commas(i)?;
			let (i, v) = preceded(key_vals, point_vals)(i)?;
			Ok((i, v))
		},
		|i| {
			let (i, v) = preceded(key_vals, point_vals)(i)?;
			let (i, _) = commas(i)?;
			let (i, _) = preceded(key_type, point_type)(i)?;
			Ok((i, v))
		},
	))(i)?;
	Ok((i, v.into()))
}

fn line(i: &str) -> IResult<&str, Geometry> {
	let (i, v) = alt((
		|i| {
			let (i, _) = preceded(key_type, line_type)(i)?;
			let (i, _) = commas(i)?;
			let (i, v) = preceded(key_vals, line_vals)(i)?;
			Ok((i, v))
		},
		|i| {
			let (i, v) = preceded(key_vals, line_vals)(i)?;
			let (i, _) = commas(i)?;
			let (i, _) = preceded(key_type, line_type)(i)?;
			Ok((i, v))
		},
	))(i)?;
	Ok((i, v.into()))
}

fn polygon(i: &str) -> IResult<&str, Geometry> {
	let (i, v) = alt((
		|i| {
			let (i, _) = preceded(key_type, polygon_type)(i)?;
			let (i, _) = commas(i)?;
			let (i, v) = preceded(key_vals, polygon_vals)(i)?;
			Ok((i, v))
		},
		|i| {
			let (i, v) = preceded(key_vals, polygon_vals)(i)?;
			let (i, _) = commas(i)?;
			let (i, _) = preceded(key_type, polygon_type)(i)?;
			Ok((i, v))
		},
	))(i)?;
	Ok((i, v.into()))
}

fn multipoint(i: &str) -> IResult<&str, Geometry> {
	let (i, v) = alt((
		|i| {
			let (i, _) = preceded(key_type, multipoint_type)(i)?;
			let (i, _) = commas(i)?;
			let (i, v) = preceded(key_vals, multipoint_vals)(i)?;
			Ok((i, v))
		},
		|i| {
			let (i, v) = preceded(key_vals, multipoint_vals)(i)?;
			let (i, _) = commas(i)?;
			let (i, _) = preceded(key_type, multipoint_type)(i)?;
			Ok((i, v))
		},
	))(i)?;
	Ok((i, v.into()))
}

fn multiline(i: &str) -> IResult<&str, Geometry> {
	let (i, v) = alt((
		|i| {
			let (i, _) = preceded(key_type, multiline_type)(i)?;
			let (i, _) = commas(i)?;
			let (i, v) = preceded(key_vals, multiline_vals)(i)?;
			Ok((i, v))
		},
		|i| {
			let (i, v) = preceded(key_vals, multiline_vals)(i)?;
			let (i, _) = commas(i)?;
			let (i, _) = preceded(key_type, multiline_type)(i)?;
			Ok((i, v))
		},
	))(i)?;
	Ok((i, v.into()))
}

fn multipolygon(i: &str) -> IResult<&str, Geometry> {
	let (i, v) = alt((
		|i| {
			let (i, _) = preceded(key_type, multipolygon_type)(i)?;
			let (i, _) = commas(i)?;
			let (i, v) = preceded(key_vals, multipolygon_vals)(i)?;
			Ok((i, v))
		},
		|i| {
			let (i, v) = preceded(key_vals, multipolygon_vals)(i)?;
			let (i, _) = commas(i)?;
			let (i, _) = preceded(key_type, multipolygon_type)(i)?;
			Ok((i, v))
		},
	))(i)?;
	Ok((i, v.into()))
}

fn collection(i: &str) -> IResult<&str, Geometry> {
	let (i, v) = alt((
		|i| {
			let (i, _) = preceded(key_type, collection_type)(i)?;
			let (i, _) = commas(i)?;
			let (i, v) = preceded(key_geom, collection_vals)(i)?;
			Ok((i, v))
		},
		|i| {
			let (i, v) = preceded(key_geom, collection_vals)(i)?;
			let (i, _) = commas(i)?;
			let (i, _) = preceded(key_type, collection_type)(i)?;
			Ok((i, v))
		},
	))(i)?;
	Ok((i, v.into()))
}

//
//
//

fn point_vals(i: &str) -> IResult<&str, Point<f64>> {
	let (i, v) = coordinate(i)?;
	Ok((i, v.into()))
}

fn line_vals(i: &str) -> IResult<&str, LineString<f64>> {
	let (i, v) =
		delimited_list0(openbracket, commas, terminated(coordinate, mightbespace), char(']'))(i)?;
	Ok((i, v.into()))
}

fn polygon_vals(i: &str) -> IResult<&str, Polygon<f64>> {
	let (i, mut e) =
		delimited_list1(openbracket, commas, terminated(line_vals, mightbespace), char(']'))(i)?;
	let v = e.split_off(1);
	// delimited_list1 guarentees there is atleast one value.
	let e = e.into_iter().next().unwrap();
	Ok((i, Polygon::new(e, v)))
}

fn multipoint_vals(i: &str) -> IResult<&str, Vec<Point<f64>>> {
	let (i, v) =
		delimited_list0(openbracket, commas, terminated(point_vals, mightbespace), char(']'))(i)?;
	Ok((i, v))
}

fn multiline_vals(i: &str) -> IResult<&str, Vec<LineString<f64>>> {
	let (i, v) =
		delimited_list0(openbracket, commas, terminated(line_vals, mightbespace), char(']'))(i)?;
	Ok((i, v))
}

fn multipolygon_vals(i: &str) -> IResult<&str, Vec<Polygon<f64>>> {
	let (i, v) =
		delimited_list0(openbracket, commas, terminated(polygon_vals, mightbespace), char(']'))(i)?;
	Ok((i, v))
}

fn collection_vals(i: &str) -> IResult<&str, Vec<Geometry>> {
	let (i, v) =
		delimited_list0(openbracket, commas, terminated(geometry, mightbespace), char(']'))(i)?;
	Ok((i, v))
}

//
//
//

fn coordinate(i: &str) -> IResult<&str, (f64, f64)> {
	let (i, _) = openbracket(i)?;
	let (i, x) = double(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, _) = char(',')(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, y) = double(i)?;
	let (i, _) = closebracket(i)?;
	Ok((i, (x, y)))
}

//
//
//

fn point_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		delimited(char(SINGLE), tag("Point"), char(SINGLE)),
		delimited(char(DOUBLE), tag("Point"), char(DOUBLE)),
	))(i)?;
	Ok((i, v))
}

fn line_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		delimited(char(SINGLE), tag("LineString"), char(SINGLE)),
		delimited(char(DOUBLE), tag("LineString"), char(DOUBLE)),
	))(i)?;
	Ok((i, v))
}

fn polygon_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		delimited(char(SINGLE), tag("Polygon"), char(SINGLE)),
		delimited(char(DOUBLE), tag("Polygon"), char(DOUBLE)),
	))(i)?;
	Ok((i, v))
}

fn multipoint_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		delimited(char(SINGLE), tag("MultiPoint"), char(SINGLE)),
		delimited(char(DOUBLE), tag("MultiPoint"), char(DOUBLE)),
	))(i)?;
	Ok((i, v))
}

fn multiline_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		delimited(char(SINGLE), tag("MultiLineString"), char(SINGLE)),
		delimited(char(DOUBLE), tag("MultiLineString"), char(DOUBLE)),
	))(i)?;
	Ok((i, v))
}

fn multipolygon_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		delimited(char(SINGLE), tag("MultiPolygon"), char(SINGLE)),
		delimited(char(DOUBLE), tag("MultiPolygon"), char(DOUBLE)),
	))(i)?;
	Ok((i, v))
}

fn collection_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		delimited(char(SINGLE), tag("GeometryCollection"), char(SINGLE)),
		delimited(char(DOUBLE), tag("GeometryCollection"), char(DOUBLE)),
	))(i)?;
	Ok((i, v))
}

//
//
//

fn key_type(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		tag("type"),
		delimited(char(SINGLE), tag("type"), char(SINGLE)),
		delimited(char(DOUBLE), tag("type"), char(DOUBLE)),
	))(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, _) = char(':')(i)?;
	let (i, _) = mightbespace(i)?;
	Ok((i, v))
}

fn key_vals(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		tag("coordinates"),
		delimited(char(SINGLE), tag("coordinates"), char(SINGLE)),
		delimited(char(DOUBLE), tag("coordinates"), char(DOUBLE)),
	))(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, _) = char(':')(i)?;
	let (i, _) = mightbespace(i)?;
	Ok((i, v))
}

fn key_geom(i: &str) -> IResult<&str, &str> {
	let (i, v) = alt((
		tag("geometries"),
		delimited(char(SINGLE), tag("geometries"), char(SINGLE)),
		delimited(char(DOUBLE), tag("geometries"), char(DOUBLE)),
	))(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, _) = char(':')(i)?;
	let (i, _) = mightbespace(i)?;
	Ok((i, v))
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn simple() {
		let sql = "(-0.118092, 51.509865)";
		let res = geometry(sql);
		let out = res.unwrap().1;
		assert_eq!("(-0.118092, 51.509865)", format!("{}", out));
	}

	#[test]
	fn point() {
		let sql = r#"{
			type: 'Point',
			coordinates: [-0.118092, 51.509865]
		}"#;
		let res = geometry(sql);
		let out = res.unwrap().1;
		assert_eq!("(-0.118092, 51.509865)", format!("{}", out));
	}

	#[test]
	fn polygon_exterior() {
		let sql = r#"{
			type: 'Polygon',
			coordinates: [
				[
					[-0.38314819, 51.37692386], [0.1785278, 51.37692386],
					[0.1785278, 51.61460570], [-0.38314819, 51.61460570],
					[-0.38314819, 51.37692386]
				]
			]
		}"#;
		let res = geometry(sql);
		let out = res.unwrap().1;
		assert_eq!("{ type: 'Polygon', coordinates: [[[-0.38314819, 51.37692386], [0.1785278, 51.37692386], [0.1785278, 51.6146057], [-0.38314819, 51.6146057], [-0.38314819, 51.37692386]]] }", format!("{}", out));
	}

	#[test]
	fn polygon_interior() {
		let sql = r#"{
			type: 'Polygon',
			coordinates: [
				[
					[-0.38314819, 51.37692386], [0.1785278, 51.37692386],
					[0.1785278, 51.61460570], [-0.38314819, 51.61460570],
					[-0.38314819, 51.37692386]
				],
				[
					[-0.38314819, 51.37692386], [0.1785278, 51.37692386],
					[0.1785278, 51.61460570], [-0.38314819, 51.61460570],
					[-0.38314819, 51.37692386]
				]
			]
		}"#;
		let res = geometry(sql);
		let out = res.unwrap().1;
		assert_eq!("{ type: 'Polygon', coordinates: [[[-0.38314819, 51.37692386], [0.1785278, 51.37692386], [0.1785278, 51.6146057], [-0.38314819, 51.6146057], [-0.38314819, 51.37692386]], [[[-0.38314819, 51.37692386], [0.1785278, 51.37692386], [0.1785278, 51.6146057], [-0.38314819, 51.6146057], [-0.38314819, 51.37692386]]]] }", format!("{}", out));
	}
}
