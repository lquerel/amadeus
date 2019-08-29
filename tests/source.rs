#![allow(where_clauses_object_safety)]

#[macro_use]
extern crate serde_closure;

use amadeus::{
	prelude::*, source::aws::{CloudfrontRow, Error}
};
use constellation::*;
use std::{
	env, time::{Duration, SystemTime}
};
use warc_parser::WebpageOwned;

fn main() {
	init(Resources::default());

	// Accept the number of processes at the command line, defaulting to 10
	let processes = env::args()
		.nth(1)
		.and_then(|arg| arg.parse::<usize>().ok())
		.unwrap_or(10);

	let start = SystemTime::now();

	let pool = ProcessPool::new(processes, Resources::default()).unwrap();

	CommonCrawl::new("CC-MAIN-2018-43").unwrap().all(
		&pool,
		FnMut!([start] move |x: Result<WebpageOwned,_>| -> bool {
			println!("{}", x.unwrap().url);
			start.elapsed().unwrap() < Duration::new(10,0)
		}),
	);

	let _ = DistributedIteratorMulti::<&Result<CloudfrontRow, Error>>::count(Identity);

	let ((), (count, count2)) = Cloudfront::new(
		rusoto_core::Region::UsEast1,
		"us-east-1.data-analytics",
		"cflogworkshop/raw/cf-accesslogs",
	)
	.unwrap()
	.multi(
		&pool,
		Identity.for_each(FnMut!(|x: Result<CloudfrontRow, _>| {
			println!("{:?}", x.unwrap().url);
		})),
		(
			Identity.map(FnMut!(|_x: &Result<_, _>| {})).count(),
			Identity.cloned().count(),
			// DistributedIteratorMulti::<&Result<CloudfrontRow, Error>>::count(Identity),
		),
	);
	assert_eq!(count, count2);
	assert_eq!(count, 207_928);
}
