use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use std::{
	pin::Pin, task::{Context, Poll}
};

use super::{
	Consumer, ConsumerAsync, ConsumerMulti, ConsumerMultiAsync, DistributedIterator, DistributedIteratorMulti
};
use crate::{
	pool::ProcessSend, sink::{Sink, SinkMap}
};

#[must_use]
pub struct Update<I, F> {
	i: I,
	f: F,
}
impl<I, F> Update<I, F> {
	pub(super) fn new(i: I, f: F) -> Self {
		Self { i, f }
	}
}

impl<I: DistributedIterator, F> DistributedIterator for Update<I, F>
where
	F: FnMut(&mut I::Item) + Clone + ProcessSend,
{
	type Item = I::Item;
	type Task = UpdateConsumer<I::Task, F>;

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.i.size_hint()
	}
	fn next_task(&mut self) -> Option<Self::Task> {
		self.i.next_task().map(|task| {
			let f = self.f.clone();
			UpdateConsumer { task, f }
		})
	}
}

impl<I: DistributedIteratorMulti<Source>, F, Source> DistributedIteratorMulti<Source>
	for Update<I, F>
where
	F: FnMut(&mut <I as DistributedIteratorMulti<Source>>::Item) + Clone + ProcessSend,
{
	type Item = I::Item;
	type Task = UpdateConsumer<I::Task, F>;

	fn task(&self) -> Self::Task {
		let task = self.i.task();
		let f = self.f.clone();
		UpdateConsumer { task, f }
	}
}

#[pin_project]
#[derive(Serialize, Deserialize)]
pub struct UpdateConsumer<T, F> {
	#[pin]
	task: T,
	f: F,
}

impl<C: Consumer, F> Consumer for UpdateConsumer<C, F>
where
	F: FnMut(&mut C::Item) + Clone,
{
	type Item = C::Item;
	type Async = UpdateConsumer<C::Async, F>;
	fn into_async(self) -> Self::Async {
		UpdateConsumer {
			task: self.task.into_async(),
			f: self.f,
		}
	}
}
impl<C: ConsumerMulti<Source>, F, Source> ConsumerMulti<Source> for UpdateConsumer<C, F>
where
	F: FnMut(&mut <C as ConsumerMulti<Source>>::Item) + Clone,
{
	type Item = C::Item;
	type Async = UpdateConsumer<C::Async, F>;
	fn into_async(self) -> Self::Async {
		UpdateConsumer {
			task: self.task.into_async(),
			f: self.f,
		}
	}
}

impl<C: ConsumerAsync, F> ConsumerAsync for UpdateConsumer<C, F>
where
	F: FnMut(&mut C::Item) + Clone,
{
	type Item = C::Item;

	fn poll_run(
		self: Pin<&mut Self>, cx: &mut Context, sink: &mut impl Sink<Self::Item>,
	) -> Poll<bool> {
		let mut self_ = self.project();
		let (task, f) = (self_.task, &mut self_.f);
		task.poll_run(
			cx,
			&mut SinkMap::new(sink, |mut item| {
				f(&mut item);
				item
			}),
		)
	}
}

impl<C: ConsumerMultiAsync<Source>, F, Source> ConsumerMultiAsync<Source> for UpdateConsumer<C, F>
where
	F: FnMut(&mut <C as ConsumerMultiAsync<Source>>::Item) + Clone,
{
	type Item = C::Item;

	fn poll_run(
		self: Pin<&mut Self>, cx: &mut Context, source: Option<Source>,
		sink: &mut impl Sink<Self::Item>,
	) -> Poll<bool> {
		let mut self_ = self.project();
		let (task, f) = (self_.task, &mut self_.f);
		task.poll_run(
			cx,
			source,
			&mut SinkMap::new(sink, |mut item| {
				f(&mut item);
				item
			}),
		)
	}
}
