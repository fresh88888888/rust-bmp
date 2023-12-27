

fn main() {
    
}

struct Node<'a>{
    prev: Option<&'a mut Node<'a>>,
    next: Option<&'a mut Node<'a>>,
    data: i64,
    start_end: bool,
}

impl <'a> Node<'a> {
    pub fn new(i: i64) -> Node<'a> {
        Node { prev: None, next: None, data: i, start_end: false }
    }

    pub fn new_start_end() -> Node<'a> {
        Node { prev: None, next: None, data: 0, start_end: true }
    }
}

struct List<'a> {
    start: Option<&'a mut Node<'a>>,
    end: Option<&'a mut Node<'a>>,
}

/* 
impl <'a> List<'a> {
    pub fn new() -> List<'a> {
        let mut lst = List{ start: Node::new_start_end(), end: Node::new_start_end() };
        lst.start.unwrap().prev = Some()
    }

    pub fn push_front(&mut self, i: i64) {
        let mut n = Box::new(Node::new(i));
        n.next = self.start;
        self.start = Some(&mut n);

    }
}

*/