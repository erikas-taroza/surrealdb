use crate::err::Error;
use crate::sql::ending::number as ending;
use crate::sql::error::Error::Parser;
use crate::sql::error::IResult;
use crate::sql::strand::Strand;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::i64;
use nom::combinator::{opt, value};
use nom::number::complete::recognize_float;
use nom::Err::Failure;
use revision::revisioned;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::hash;
use std::iter::Product;
use std::iter::Sum;
use std::ops::{self, Neg};
use std::str::FromStr;

pub(crate) const TOKEN: &str = "$surrealdb::private::sql::Number";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename = "$surrealdb::private::sql::Number")]
#[revisioned(revision = 1)]
pub enum Number {
	Int(i64),
	Float(f64),
	Decimal(Decimal),
	// Add new variants here
}

impl Default for Number {
	fn default() -> Self {
		Self::Int(0)
	}
}

macro_rules! from_prim_ints {
	($($int: ty),*) => {
		$(
			impl From<$int> for Number {
				fn from(i: $int) -> Self {
					Self::Int(i as i64)
				}
			}
		)*
	};
}

from_prim_ints!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

impl From<f32> for Number {
	fn from(f: f32) -> Self {
		Self::Float(f as f64)
	}
}

impl From<f64> for Number {
	fn from(f: f64) -> Self {
		Self::Float(f)
	}
}

impl From<Decimal> for Number {
	fn from(v: Decimal) -> Self {
		Self::Decimal(v)
	}
}

impl FromStr for Number {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::try_from(s)
	}
}

impl TryFrom<String> for Number {
	type Error = ();
	fn try_from(v: String) -> Result<Self, Self::Error> {
		Self::try_from(v.as_str())
	}
}

impl TryFrom<Strand> for Number {
	type Error = ();
	fn try_from(v: Strand) -> Result<Self, Self::Error> {
		Self::try_from(v.as_str())
	}
}

impl TryFrom<&str> for Number {
	type Error = ();
	fn try_from(v: &str) -> Result<Self, Self::Error> {
		// Attempt to parse as i64
		match v.parse::<i64>() {
			// Store it as an i64
			Ok(v) => Ok(Self::Int(v)),
			// It wasn't parsed as a i64 so parse as a float
			_ => match f64::from_str(v) {
				// Store it as a float
				Ok(v) => Ok(Self::Float(v)),
				// It wasn't parsed as a number
				_ => Err(()),
			},
		}
	}
}

macro_rules! try_into_prim {
	// TODO: switch to one argument per int once https://github.com/rust-lang/rust/issues/29599 is stable
	($($int: ty => $to_int: ident),*) => {
		$(
			impl TryFrom<Number> for $int {
				type Error = Error;
				fn try_from(value: Number) -> Result<Self, Self::Error> {
					match value {
						Number::Int(v) => match v.$to_int() {
							Some(v) => Ok(v),
							None => Err(Error::TryFrom(value.to_string(), stringify!($int))),
						},
						Number::Float(v) => match v.$to_int() {
							Some(v) => Ok(v),
							None => Err(Error::TryFrom(value.to_string(), stringify!($int))),
						},
						Number::Decimal(ref v) => match v.$to_int() {
							Some(v) => Ok(v),
							None => Err(Error::TryFrom(value.to_string(), stringify!($int))),
						},
					}
				}
			}
		)*
	};
}

try_into_prim!(
	i8 => to_i8, i16 => to_i16, i32 => to_i32, i64 => to_i64, i128 => to_i128,
	u8 => to_u8, u16 => to_u16, u32 => to_u32, u64 => to_u64, u128 => to_u128,
	f32 => to_f32, f64 => to_f64
);

impl TryFrom<Number> for Decimal {
	type Error = Error;
	fn try_from(value: Number) -> Result<Self, Self::Error> {
		match value {
			Number::Int(v) => match Decimal::from_i64(v) {
				Some(v) => Ok(v),
				None => Err(Error::TryFrom(value.to_string(), "Decimal")),
			},
			Number::Float(v) => match Decimal::try_from(v) {
				Ok(v) => Ok(v),
				_ => Err(Error::TryFrom(value.to_string(), "Decimal")),
			},
			Number::Decimal(x) => Ok(x),
		}
	}
}

impl Display for Number {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		match self {
			Number::Int(v) => Display::fmt(v, f),
			Number::Float(v) => {
				if v.is_finite() {
					// Add suffix to distinguish between int and float
					write!(f, "{v}f")
				} else {
					// Don't add suffix for NaN, inf, -inf
					Display::fmt(v, f)
				}
			}
			Number::Decimal(v) => write!(f, "{v}dec"),
		}
	}
}

impl Number {
	// -----------------------------------
	// Constants
	// -----------------------------------

	pub const NAN: Number = Number::Float(f64::NAN);

	// -----------------------------------
	// Simple number detection
	// -----------------------------------

	pub fn is_nan(&self) -> bool {
		matches!(self, Number::Float(v) if v.is_nan())
	}

	pub fn is_int(&self) -> bool {
		matches!(self, Number::Int(_))
	}

	pub fn is_float(&self) -> bool {
		matches!(self, Number::Float(_))
	}

	pub fn is_decimal(&self) -> bool {
		matches!(self, Number::Decimal(_))
	}

	pub fn is_integer(&self) -> bool {
		match self {
			Number::Int(_) => true,
			Number::Float(v) => v.fract() == 0.0,
			Number::Decimal(v) => v.is_integer(),
		}
	}

	pub fn is_truthy(&self) -> bool {
		match self {
			Number::Int(v) => v != &0,
			Number::Float(v) => v != &0.0,
			Number::Decimal(v) => v != &Decimal::ZERO,
		}
	}

	pub fn is_positive(&self) -> bool {
		match self {
			Number::Int(v) => v > &0,
			Number::Float(v) => v > &0.0,
			Number::Decimal(v) => v > &Decimal::ZERO,
		}
	}

	pub fn is_negative(&self) -> bool {
		match self {
			Number::Int(v) => v < &0,
			Number::Float(v) => v < &0.0,
			Number::Decimal(v) => v < &Decimal::ZERO,
		}
	}

	pub fn is_zero(&self) -> bool {
		match self {
			Number::Int(v) => v == &0,
			Number::Float(v) => v == &0.0,
			Number::Decimal(v) => v == &Decimal::ZERO,
		}
	}

	pub fn is_zero_or_positive(&self) -> bool {
		match self {
			Number::Int(v) => v >= &0,
			Number::Float(v) => v >= &0.0,
			Number::Decimal(v) => v >= &Decimal::ZERO,
		}
	}

	pub fn is_zero_or_negative(&self) -> bool {
		match self {
			Number::Int(v) => v <= &0,
			Number::Float(v) => v <= &0.0,
			Number::Decimal(v) => v <= &Decimal::ZERO,
		}
	}

	// -----------------------------------
	// Simple conversion of number
	// -----------------------------------

	pub fn as_usize(self) -> usize {
		match self {
			Number::Int(v) => v as usize,
			Number::Float(v) => v as usize,
			Number::Decimal(v) => v.try_into().unwrap_or_default(),
		}
	}

	pub fn as_int(self) -> i64 {
		match self {
			Number::Int(v) => v,
			Number::Float(v) => v as i64,
			Number::Decimal(v) => v.try_into().unwrap_or_default(),
		}
	}

	pub fn as_float(self) -> f64 {
		match self {
			Number::Int(v) => v as f64,
			Number::Float(v) => v,
			Number::Decimal(v) => v.try_into().unwrap_or_default(),
		}
	}

	pub fn as_decimal(self) -> Decimal {
		match self {
			Number::Int(v) => Decimal::from(v),
			Number::Float(v) => Decimal::try_from(v).unwrap_or_default(),
			Number::Decimal(v) => v,
		}
	}

	// -----------------------------------
	// Complex conversion of number
	// -----------------------------------

	pub fn to_usize(&self) -> usize {
		match self {
			Number::Int(v) => *v as usize,
			Number::Float(v) => *v as usize,
			Number::Decimal(v) => v.to_usize().unwrap_or_default(),
		}
	}

	pub fn to_int(&self) -> i64 {
		match self {
			Number::Int(v) => *v,
			Number::Float(v) => *v as i64,
			Number::Decimal(v) => v.to_i64().unwrap_or_default(),
		}
	}

	pub fn to_float(&self) -> f64 {
		match self {
			Number::Int(v) => *v as f64,
			Number::Float(v) => *v,
			&Number::Decimal(v) => v.try_into().unwrap_or_default(),
		}
	}

	pub fn to_decimal(&self) -> Decimal {
		match self {
			Number::Int(v) => Decimal::try_from(*v).unwrap_or_default(),
			Number::Float(v) => Decimal::try_from(*v).unwrap_or_default(),
			Number::Decimal(v) => *v,
		}
	}

	// -----------------------------------
	//
	// -----------------------------------

	pub fn abs(self) -> Self {
		match self {
			Number::Int(v) => v.abs().into(),
			Number::Float(v) => v.abs().into(),
			Number::Decimal(v) => v.abs().into(),
		}
	}

	pub fn acos(self) -> Self {
		self.to_float().acos().into()
	}

	pub fn ceil(self) -> Self {
		match self {
			Number::Int(v) => v.into(),
			Number::Float(v) => v.ceil().into(),
			Number::Decimal(v) => v.ceil().into(),
		}
	}

	pub fn floor(self) -> Self {
		match self {
			Number::Int(v) => v.into(),
			Number::Float(v) => v.floor().into(),
			Number::Decimal(v) => v.floor().into(),
		}
	}

	pub fn round(self) -> Self {
		match self {
			Number::Int(v) => v.into(),
			Number::Float(v) => v.round().into(),
			Number::Decimal(v) => v.round().into(),
		}
	}

	pub fn fixed(self, precision: usize) -> Number {
		match self {
			Number::Int(v) => format!("{v:.precision$}").try_into().unwrap_or_default(),
			Number::Float(v) => format!("{v:.precision$}").try_into().unwrap_or_default(),
			Number::Decimal(v) => v.round_dp(precision as u32).into(),
		}
	}

	pub fn sqrt(self) -> Self {
		match self {
			Number::Int(v) => (v as f64).sqrt().into(),
			Number::Float(v) => v.sqrt().into(),
			Number::Decimal(v) => v.sqrt().unwrap_or_default().into(),
		}
	}

	pub fn pow(self, power: Number) -> Number {
		match (self, power) {
			(Number::Int(v), Number::Int(p)) => Number::Int(v.pow(p as u32)),
			(Number::Decimal(v), Number::Int(p)) => v.powi(p).into(),
			// TODO: (Number::Decimal(v), Number::Float(p)) => todo!(),
			// TODO: (Number::Decimal(v), Number::Decimal(p)) => todo!(),
			(v, p) => v.as_float().powf(p.as_float()).into(),
		}
	}
}

impl Eq for Number {}

impl Ord for Number {
	fn cmp(&self, other: &Self) -> Ordering {
		fn total_cmp_f64(a: f64, b: f64) -> Ordering {
			if a == 0.0 && b == 0.0 {
				// -0.0 = 0.0
				Ordering::Equal
			} else {
				// Handles NaN's
				a.total_cmp(&b)
			}
		}

		match (self, other) {
			(Number::Int(v), Number::Int(w)) => v.cmp(w),
			(Number::Float(v), Number::Float(w)) => total_cmp_f64(*v, *w),
			(Number::Decimal(v), Number::Decimal(w)) => v.cmp(w),
			// ------------------------------
			(Number::Int(v), Number::Float(w)) => total_cmp_f64(*v as f64, *w),
			(Number::Float(v), Number::Int(w)) => total_cmp_f64(*v, *w as f64),
			// ------------------------------
			(Number::Int(v), Number::Decimal(w)) => Decimal::from(*v).cmp(w),
			(Number::Decimal(v), Number::Int(w)) => v.cmp(&Decimal::from(*w)),
			// ------------------------------
			(Number::Float(v), Number::Decimal(w)) => {
				// `rust_decimal::Decimal` code comments indicate that `to_f64` is infallible
				total_cmp_f64(*v, w.to_f64().unwrap())
			}
			(Number::Decimal(v), Number::Float(w)) => total_cmp_f64(v.to_f64().unwrap(), *w),
		}
	}
}

// Warning: Equal numbers may have different hashes, which violates
// the invariants of certain collections!
impl hash::Hash for Number {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		match self {
			Number::Int(v) => v.hash(state),
			Number::Float(v) => v.to_bits().hash(state),
			Number::Decimal(v) => v.hash(state),
		}
	}
}

impl PartialEq for Number {
	fn eq(&self, other: &Self) -> bool {
		fn total_eq_f64(a: f64, b: f64) -> bool {
			a.to_bits().eq(&b.to_bits()) || (a == 0.0 && b == 0.0)
		}

		match (self, other) {
			(Number::Int(v), Number::Int(w)) => v.eq(w),
			(Number::Float(v), Number::Float(w)) => total_eq_f64(*v, *w),
			(Number::Decimal(v), Number::Decimal(w)) => v.eq(w),
			// ------------------------------
			(Number::Int(v), Number::Float(w)) => total_eq_f64(*v as f64, *w),
			(Number::Float(v), Number::Int(w)) => total_eq_f64(*v, *w as f64),
			// ------------------------------
			(Number::Int(v), Number::Decimal(w)) => Decimal::from(*v).eq(w),
			(Number::Decimal(v), Number::Int(w)) => v.eq(&Decimal::from(*w)),
			// ------------------------------
			(Number::Float(v), Number::Decimal(w)) => total_eq_f64(*v, w.to_f64().unwrap()),
			(Number::Decimal(v), Number::Float(w)) => total_eq_f64(v.to_f64().unwrap(), *w),
		}
	}
}

impl PartialOrd for Number {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl ops::Add for Number {
	type Output = Self;
	fn add(self, other: Self) -> Self {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v + w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v + w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v + w),
			(Number::Int(v), Number::Float(w)) => Number::Float(v as f64 + w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v + w as f64),
			(v, w) => Number::from(v.as_decimal() + w.as_decimal()),
		}
	}
}

impl<'a, 'b> ops::Add<&'b Number> for &'a Number {
	type Output = Number;
	fn add(self, other: &'b Number) -> Number {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v + w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v + w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v + w),
			(Number::Int(v), Number::Float(w)) => Number::Float(*v as f64 + w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v + *w as f64),
			(v, w) => Number::from(v.to_decimal() + w.to_decimal()),
		}
	}
}

impl ops::Sub for Number {
	type Output = Self;
	fn sub(self, other: Self) -> Self {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v - w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v - w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v - w),
			(Number::Int(v), Number::Float(w)) => Number::Float(v as f64 - w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v - w as f64),
			(v, w) => Number::from(v.as_decimal() - w.as_decimal()),
		}
	}
}

impl<'a, 'b> ops::Sub<&'b Number> for &'a Number {
	type Output = Number;
	fn sub(self, other: &'b Number) -> Number {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v - w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v - w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v - w),
			(Number::Int(v), Number::Float(w)) => Number::Float(*v as f64 - w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v - *w as f64),
			(v, w) => Number::from(v.to_decimal() - w.to_decimal()),
		}
	}
}

impl ops::Mul for Number {
	type Output = Self;
	fn mul(self, other: Self) -> Self {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v * w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v * w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v * w),
			(Number::Int(v), Number::Float(w)) => Number::Float(v as f64 * w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v * w as f64),
			(v, w) => Number::from(v.as_decimal() * w.as_decimal()),
		}
	}
}

impl<'a, 'b> ops::Mul<&'b Number> for &'a Number {
	type Output = Number;
	fn mul(self, other: &'b Number) -> Number {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v * w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v * w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v * w),
			(Number::Int(v), Number::Float(w)) => Number::Float(*v as f64 * w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v * *w as f64),
			(v, w) => Number::from(v.to_decimal() * w.to_decimal()),
		}
	}
}

impl ops::Div for Number {
	type Output = Self;
	fn div(self, other: Self) -> Self {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v / w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v / w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v / w),
			(Number::Int(v), Number::Float(w)) => Number::Float(v as f64 / w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v / w as f64),
			(v, w) => Number::from(v.as_decimal() / w.as_decimal()),
		}
	}
}

impl<'a, 'b> ops::Div<&'b Number> for &'a Number {
	type Output = Number;
	fn div(self, other: &'b Number) -> Number {
		match (self, other) {
			(Number::Int(v), Number::Int(w)) => Number::Int(v / w),
			(Number::Float(v), Number::Float(w)) => Number::Float(v / w),
			(Number::Decimal(v), Number::Decimal(w)) => Number::Decimal(v / w),
			(Number::Int(v), Number::Float(w)) => Number::Float(*v as f64 / w),
			(Number::Float(v), Number::Int(w)) => Number::Float(v / *w as f64),
			(v, w) => Number::from(v.to_decimal() / w.to_decimal()),
		}
	}
}

impl Neg for Number {
	type Output = Self;

	fn neg(self) -> Self::Output {
		match self {
			Self::Int(n) => Number::Int(-n),
			Self::Float(n) => Number::Float(-n),
			Self::Decimal(n) => Number::Decimal(-n),
		}
	}
}

// ------------------------------

impl Sum<Self> for Number {
	fn sum<I>(iter: I) -> Number
	where
		I: Iterator<Item = Self>,
	{
		iter.fold(Number::Int(0), |a, b| a + b)
	}
}

impl<'a> Sum<&'a Self> for Number {
	fn sum<I>(iter: I) -> Number
	where
		I: Iterator<Item = &'a Self>,
	{
		iter.fold(Number::Int(0), |a, b| &a + b)
	}
}

impl Product<Self> for Number {
	fn product<I>(iter: I) -> Number
	where
		I: Iterator<Item = Self>,
	{
		iter.fold(Number::Int(1), |a, b| a * b)
	}
}

impl<'a> Product<&'a Self> for Number {
	fn product<I>(iter: I) -> Number
	where
		I: Iterator<Item = &'a Self>,
	{
		iter.fold(Number::Int(1), |a, b| &a * b)
	}
}

pub struct Sorted<T>(pub T);

pub trait Sort {
	fn sorted(&mut self) -> Sorted<&Self>
	where
		Self: Sized;
}

impl Sort for Vec<Number> {
	fn sorted(&mut self) -> Sorted<&Vec<Number>> {
		self.sort();
		Sorted(self)
	}
}

fn not_nan(i: &str) -> IResult<&str, Number> {
	let (i, v) = recognize_float(i)?;
	let (i, suffix) = suffix(i)?;
	let (i, _) = ending(i)?;
	let number = match suffix {
		Suffix::None => Number::try_from(v).map_err(|_| Failure(Parser(i)))?,
		Suffix::Float => Number::from(f64::from_str(v).map_err(|_| Failure(Parser(i)))?),
		Suffix::Decimal => Number::from(Decimal::from_str(v).map_err(|_| Failure(Parser(i)))?),
	};
	Ok((i, number))
}

pub fn number(i: &str) -> IResult<&str, Number> {
	alt((value(Number::NAN, tag("NaN")), not_nan))(i)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Suffix {
	None,
	Float,
	Decimal,
}

fn suffix(i: &str) -> IResult<&str, Suffix> {
	let (i, opt_suffix) =
		opt(alt((value(Suffix::Float, tag("f")), value(Suffix::Decimal, tag("dec")))))(i)?;
	Ok((i, opt_suffix.unwrap_or(Suffix::None)))
}

pub fn integer(i: &str) -> IResult<&str, i64> {
	let (i, v) = i64(i)?;
	let (i, _) = ending(i)?;
	Ok((i, v))
}

#[cfg(test)]
mod tests {

	use super::*;
	use std::ops::Div;

	#[test]
	fn number_nan() {
		let sql = "NaN";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!("NaN", format!("{}", out));
	}

	#[test]
	fn number_int() {
		let sql = "123";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!("123", format!("{}", out));
		assert_eq!(out, Number::Int(123));
	}

	#[test]
	fn number_int_neg() {
		let sql = "-123";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!("-123", format!("{}", out));
		assert_eq!(out, Number::Int(-123));
	}

	#[test]
	fn number_float() {
		let sql = "123.45f";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!(sql, format!("{}", out));
		assert_eq!(out, Number::Float(123.45));
	}

	#[test]
	fn number_float_neg() {
		let sql = "-123.45f";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!(sql, format!("{}", out));
		assert_eq!(out, Number::Float(-123.45));
	}

	#[test]
	fn number_scientific_lower() {
		let sql = "12345e-1";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!("1234.5f", format!("{}", out));
		assert_eq!(out, Number::Float(1234.5));
	}

	#[test]
	fn number_scientific_lower_neg() {
		let sql = "-12345e-1";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!("-1234.5f", format!("{}", out));
		assert_eq!(out, Number::Float(-1234.5));
	}

	#[test]
	fn number_scientific_upper() {
		let sql = "12345E-02";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!("123.45f", format!("{}", out));
		assert_eq!(out, Number::Float(123.45));
	}

	#[test]
	fn number_scientific_upper_neg() {
		let sql = "-12345E-02";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!("-123.45f", format!("{}", out));
		assert_eq!(out, Number::Float(-123.45));
	}

	#[test]
	fn number_float_keeps_precision() {
		let sql = "13.571938471938472f";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!(sql, format!("{}", out));
	}

	#[test]
	fn number_decimal_keeps_precision() {
		let sql = "0.0000000000000000000000000321dec";
		let res = number(sql);
		let out = res.unwrap().1;
		assert_eq!(sql, format!("{}", out));
	}

	#[test]
	fn number_div_int() {
		let res = Number::Int(3).div(Number::Int(2));
		assert_eq!(res, Number::Int(1));
	}

	#[test]
	fn number_pow_int() {
		let res = Number::Int(3).pow(Number::Int(4));
		assert_eq!(res, Number::Int(81));
	}

	#[test]
	fn number_pow_int_negative() {
		let res = Number::Int(4).pow(Number::Float(-0.5));
		assert_eq!(res, Number::Float(0.5));
	}

	#[test]
	fn number_pow_float() {
		let res = Number::Float(2.5).pow(Number::Int(2));
		assert_eq!(res, Number::Float(6.25));
	}

	#[test]
	fn number_pow_float_negative() {
		let res = Number::Int(4).pow(Number::Float(-0.5));
		assert_eq!(res, Number::Float(0.5));
	}

	#[test]
	fn number_pow_decimal_one() {
		let res = Number::try_from("13.5719384719384719385639856394139476937756394756")
			.unwrap()
			.pow(Number::Int(1));
		assert_eq!(
			res,
			Number::try_from("13.5719384719384719385639856394139476937756394756").unwrap()
		);
	}

	#[test]
	fn number_pow_decimal_two() {
		let res = Number::try_from("13.5719384719384719385639856394139476937756394756")
			.unwrap()
			.pow(Number::Int(2));
		assert_eq!(
			res,
			Number::try_from("184.19751388608358465578173996877942643463869043732548087725588482334195240945031617770904299536").unwrap()
		);
	}

	#[test]
	fn ord() {
		fn assert_cmp(a: &Number, b: &Number, ord: Ordering) {
			assert_eq!(a.cmp(b), ord, "{a} {ord:?} {b}");
			assert_eq!(a == b, ord.is_eq(), "{a} {ord:?} {b}");
		}

		let nz = -0.0f64;
		let z = 0.0f64;
		assert_ne!(nz.to_bits(), z.to_bits());
		let nzp = permutations(nz);
		let zp = permutations(z);
		for nzp in nzp.iter() {
			for zp in zp.iter() {
				assert_cmp(nzp, zp, Ordering::Equal);
			}
		}

		let negative_nan = f64::from_bits(18444492273895866368);

		let ordering = &[
			negative_nan,
			f64::NEG_INFINITY,
			-10.0,
			-1.0,
			-f64::MIN_POSITIVE,
			0.0,
			f64::MIN_POSITIVE,
			1.0,
			10.0,
			f64::INFINITY,
			f64::NAN,
		];

		fn permutations(n: f64) -> Vec<Number> {
			let mut ret = Vec::new();
			ret.push(Number::Float(n));
			if n.is_finite() && (n == 0.0 || n.abs() > f64::EPSILON) {
				ret.push(Number::Decimal(n.try_into().unwrap()));
				ret.push(Number::Int(n as i64));
			}
			ret
		}

		for (ai, a) in ordering.iter().enumerate() {
			let ap = permutations(*a);
			for (bi, b) in ordering.iter().enumerate() {
				let bp = permutations(*b);
				let correct = ai.cmp(&bi);

				for a in &ap {
					for b in &bp {
						assert_cmp(a, b, correct);
					}
				}
			}
		}
	}
}
