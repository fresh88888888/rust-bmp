fn main() {
    let mut lst = List::new();
    lst.push_front(1);
    lst.push_front(2);
    lst.push_front(3);
    lst.push_front(4);

    lst.print();
}

struct Node {
    next: Option<Box<Self>>,
    data: i64,
}

impl Node {
    pub const fn new(i: i64) -> Self {
        Self {
            next: None,
            data: i,
        }
    }
}

struct List {
    start: Option<Box<Node>>,
    //end: Option<Box<Node>>,
}

impl List {
    pub const fn new() -> Self {
        Self {
            start: None,
        }
    }

    pub fn push_front(&mut self, i: i64) {
        let mut n = Box::new(Node::new(i));
        n.next = self.start.take();
        self.start = Some(n);
    }

    pub fn print(&self) {
        let mut ptr = &self.start;
        while let Some(p) = ptr {
            println!("{}", p.data);
            ptr = &p.next;
        }
    }
}
