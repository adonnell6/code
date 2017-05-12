//! Lock-free stacks.
//!
//! This code is based on [an article by Aaron
//! Turon](https://aturon.github.io/blog/2015/08/27/epoch/).

extern crate crossbeam;

use std::ptr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Acquire, Release, Relaxed};

use self::crossbeam::mem::epoch::{self, Atomic, Owned};

/// A lock-free stack.
pub struct Stack<T> {
    head: Atomic<Node<T>>,
    len:  AtomicUsize,
}

struct Node<T> {
    data: T,
    next: Atomic<Node<T>>,
}

impl<T> Stack<T> {
    /// Returns a new, empty stack.
    pub fn new() -> Stack<T> {
        Stack {
            head: Atomic::null(),
            len:  AtomicUsize::new(0),
        }
    }

    /// Checks whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.head.load(Acquire, &epoch::pin()).is_none()
    }

    /// Returns a snapshop of the number of elements in the stack.
    pub fn len(&self) -> usize {
        self.len.load(Relaxed)
    }

    /// Pushes an element on top of the stack.
    pub fn push(&self, data: T) {
        let mut new_node = Owned::new(Node {
            data: data,
            next: Atomic::null(),
        });

        let guard = epoch::pin();

        loop {
            let head = self.head.load(Acquire, &guard);
            new_node.next.store_shared(head, Relaxed);

            match self.head.cas(head, Some(new_node), Release) {
                Ok(_) => {
                    self.len.fetch_add(1, Relaxed);
                    return;
                }
                Err(owned) => new_node = owned.unwrap(),
            }
        }
    }

    /// Removes and returns the top element of the stack, or `None` if
    /// empty.
    pub fn pop(&self) -> Option<T> {
        let guard = epoch::pin();

        loop {
            if let Some(head) = self.head.load(Acquire, &guard) {
                let next = head.next.load(Relaxed, &guard);

                if self.head.cas_shared(Some(head), next, Release) {
                    self.len.fetch_sub(1, Relaxed);
                    return Some(unsafe {
                        guard.unlinked(head);
                        ptr::read(&head.data)
                    });
                }
            } else {
                return None;
            }
        }
    }
}

impl<T: Clone> Stack<T> {
    /// Gets a clone of the top element of the stack, if there is one.
    pub fn peek(&self) -> Option<T> {
        let guard = epoch::pin();
        self.head.load(Acquire, &guard).map(|head| head.data.clone())
    }
}
