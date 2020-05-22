use futures::stream;
use serde::{Deserialize, Serialize};
use serde_closure::*;
use serde_json::Error as SerdeJsonError;
use std::{
	error, fmt::{self, Debug, Display}, io::BufReader, iter, marker::PhantomData
};

use amadeus_core::{
	dist_iter::DistributedIterator, file::{File, Page, Partition}, into_dist_iter::IntoDistributedIterator, util::ResultExpand, Source
};

use super::{SerdeData, SerdeDeserialize};

#[derive(Clone)]
pub struct Json<File, Row>
where
	File: amadeus_core::file::File,
	Row: SerdeData,
{
	partitions: Vec<File::Partition>,
	marker: PhantomData<fn() -> Row>,
}
impl<F, Row> Json<F, Row>
where
	F: File,
	Row: SerdeData,
{
	pub fn new(file: F) -> Result<Self, <Self as Source>::Error> {
		Ok(Self {
			partitions: file.partitions().map_err(JsonError::File)?,
			marker: PhantomData,
		})
	}
}
impl<F, Row> Source for Json<F, Row>
where
	F: File,
	Row: SerdeData,
{
	type Item = Row;
	#[allow(clippy::type_complexity)]
	type Error = JsonError<
		<F as File>::Error,
		<<F as File>::Partition as Partition>::Error,
		<<<F as File>::Partition as Partition>::Page as Page>::Error,
	>;

	#[cfg(not(feature = "doc"))]
	type DistIter = impl DistributedIterator<Item = Result<Self::Item, Self::Error>>;
	#[cfg(feature = "doc")]
	type DistIter = amadeus_core::util::ImplDistributedIterator<Result<Self::Item, Self::Error>>;
	type Iter = iter::Empty<Result<Self::Item, Self::Error>>;

	#[allow(clippy::let_and_return)]
	fn dist_iter(self) -> Self::DistIter {
		let ret = self
			.partitions
			.into_dist_iter()
			.flat_map(FnMut!(|partition: F::Partition| {
				stream::iter(
					ResultExpand(partition.pages().map_err(JsonError::Partition))
						.into_iter()
						.flat_map(|page: Result<_, _>| {
							ResultExpand(page.map(|page| {
								let reader = BufReader::new(Page::reader(page));
								serde_json::Deserializer::from_reader(reader)
									.into_iter()
									.map(
										|x: Result<SerdeDeserialize<Row>, SerdeJsonError>| Ok(x?.0),
									)
							}))
						})
						.map(|row: Result<Result<Row, SerdeJsonError>, Self::Error>| Ok(row??)),
				)
			}));
		#[cfg(feature = "doc")]
		let ret = amadeus_core::util::ImplDistributedIterator::new(ret);
		ret
	}
	fn iter(self) -> Self::Iter {
		iter::empty()
		// self.files
		// 	.into_iter()
		// 	.flat_map(|file: PathBuf| {
		// 		let files = if !file.is_dir() {
		// 			sum::Sum2::A(iter::once(Ok(file)))
		// 		} else {
		// 			sum::Sum2::B(get_json_partitions(file))
		// 		};
		// 		files
		// 			.flat_map(|file: Result<PathBuf, _>| ResultExpand(
		// 				file.and_then(|file| Ok(fs::File::open(file)?)).map(|file| {
		// 					serde_json::Deserializer::from_reader(file)
		// 						.into_iter()
		// 						.map(FnMut!(|x: Result<SerdeDeserialize<Row>, SerdeJsonError>| Ok(
		// 							x?.0
		// 						)))
		// 				})
		// 			))
		// 			.map(|row: Result<Result<Row, SerdeJsonError>, io::Error>| Ok(row??))
		// 	})
	}
}

mod jsonerror {
	use serde::{Deserializer, Serializer};

	pub fn serialize<T, S>(_t: &T, _serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		unimplemented!()
	}
	pub fn deserialize<'de, T, D>(_deserializer: D) -> Result<T, D::Error>
	where
		D: Deserializer<'de>,
	{
		unimplemented!()
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub enum JsonError<A, B, C> {
	File(A),
	Partition(B),
	Page(C),
	Json(#[serde(with = "jsonerror")] SerdeJsonError),
}
impl<A, B, C> Clone for JsonError<A, B, C>
where
	A: Clone,
	B: Clone,
	C: Clone,
{
	fn clone(&self) -> Self {
		match self {
			Self::File(err) => Self::File(err.clone()),
			Self::Partition(err) => Self::Partition(err.clone()),
			Self::Page(err) => Self::Page(err.clone()),
			Self::Json(err) => Self::Json(serde::de::Error::custom(err)),
		}
	}
}
impl<A, B, C> PartialEq for JsonError<A, B, C>
where
	A: PartialEq,
	B: PartialEq,
	C: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::File(a), Self::File(b)) => a.eq(b),
			(Self::Partition(a), Self::Partition(b)) => a.eq(b),
			(Self::Page(a), Self::Page(b)) => a.eq(b),
			(Self::Json(a), Self::Json(b)) => a.to_string() == b.to_string(),
			_ => false,
		}
	}
}
impl<A, B, C> error::Error for JsonError<A, B, C>
where
	A: error::Error,
	B: error::Error,
	C: error::Error,
{
}
impl<A, B, C> Display for JsonError<A, B, C>
where
	A: Display,
	B: Display,
	C: Display,
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::File(err) => Display::fmt(err, f),
			Self::Partition(err) => Display::fmt(err, f),
			Self::Page(err) => Display::fmt(err, f),
			Self::Json(err) => Display::fmt(err, f),
		}
	}
}
impl<A, B, C> From<SerdeJsonError> for JsonError<A, B, C> {
	fn from(err: SerdeJsonError) -> Self {
		Self::Json(err)
	}
}
