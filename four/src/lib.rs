#![deny(warnings)]
#![cfg_attr(test, deny(warnings))]

use std::cell::RefCell;
use std::rc::Rc;

type Link = Rc<RefCell<Node>>;

#[derive(Clone)]
struct Node {
    value: i32,
    prev: Option<Link>,
    next: Option<Link>,
}

pub struct List {
    size: usize,
    head: Option<Link>,
    tail: Option<Link>,
}

impl List {
    pub fn new() -> List {
        List {
            size: 0,
            head: None,
            tail: None,
        }
    }

    pub fn push_back(&mut self, value: i32) {
        let n = Node {
            value,
            prev: self.tail.clone(),
            next: None,
        };
        let n = Rc::new(RefCell::new(n));

        match self.tail {
            Some(ref pre_tail) => {
                pre_tail.borrow_mut().next = Some(Rc::clone(&n));
                n.borrow_mut().prev = Some(Rc::clone(pre_tail));
                self.tail = Some(Rc::clone(&n));
            }
            None => {
                self.head = Some(Rc::clone(&n));
                self.tail = Some(Rc::clone(self.head.as_ref().unwrap()));
            }
        }
        self.size += 1;
    }

    pub fn get(&self, index: usize) -> Option<i32> {
        self.get_link_at(index)
            .map(|node| node.as_ref().borrow().value)
    }

    fn get_link_at(&self, index: usize) -> Option<Link> {
        if index >= self.len() {
            return None;
        }

        let direction_from_head = index <= self.size / 2;
        let index = if direction_from_head {
            index
        } else {
            self.size - index - 1
        };

        let mut current: Link = match direction_from_head {
            true => Rc::clone(self.head.as_ref().unwrap()),
            false => Rc::clone(self.tail.as_ref().unwrap()),
        };

        for _ in 0..index {
            current = match direction_from_head {
                true => Rc::clone(current.as_ref().borrow().next.as_ref().unwrap()),
                false => Rc::clone(current.as_ref().borrow().prev.as_ref().unwrap()),
            };
        }

        Some(current)
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        if self.size == 0 {
            return true;
        }

        false
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests{
    //use super::*;

    //const UPPER_BOUNDS: usize = 1000;


}