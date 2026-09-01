#[derive(Debug, Copy, PartialEq, Eq, Clone)]
pub struct DLLNodeID(usize);

#[derive(Debug, Clone)]
struct DLLnode<T, O: Ord> {
    //this indexes into the string at the start of a char, the design of the program should
    //guarantee that this doesnt panic. Namely by only considering usizes that come
    //immediately from a char_indices() and only subtracting from that offset when the character before that is known.
    underlying_position: O,
    content: Option<T>,
    id_of_prev: Option<DLLNodeID>,
    id_of_next: Option<DLLNodeID>,
}

impl<T, O: Ord> DLLnode<T, O> {
    fn new(
        underlying_position: O,
        content: T,
        id_of_prev: Option<DLLNodeID>,
        id_of_next: Option<DLLNodeID>,
    ) -> Self {
        DLLnode {
            underlying_position,
            content: Some(content),
            id_of_prev,
            id_of_next,
        }
    }
}

// we will be approxiamating a double linked list in rust by having each item in the list keep
// track of the index of the next item.
// TODO: make this in to a more proper arena using ops and stuff
#[derive(Debug, Clone)]
pub struct ArenaDLL<T, O: Ord> {
    // beginning_char_offset,end_char_offset, dl, index_of_prev, index_of_next
    nodes: Vec<DLLnode<T, O>>,
    // Since we only push to the dll at the beginning, we do not need to keep track of
    // "freeing" things for later. (monotonic?)
    start_id: Option<DLLNodeID>,
    end_id: Option<DLLNodeID>,
}

impl<T, O: Ord> Default for ArenaDLL<T, O> {
    fn default() -> Self {
        ArenaDLL {
            nodes: vec![],
            start_id: None,
            end_id: None,
        }
    }
}

impl<T, O: Ord> ArenaDLL<T, O> {
    pub fn push_back(&mut self, relative_position: O, content: T) -> DLLNodeID {
        let id_of_this = DLLNodeID(self.nodes.len());
        let id_of_prev = if self.start_id.is_none() {
            self.start_id = Some(id_of_this);
            None
        } else {
            self.nodes[self.end_id.unwrap().0].id_of_next = Some(id_of_this);
            self.end_id
        };
        let id_of_next = None;
        self.nodes.push(DLLnode::new(
            relative_position,
            content,
            id_of_prev,
            id_of_next,
        ));
        self.end_id = Some(id_of_this);
        id_of_this
    }

    pub fn get_end_id(&self) -> Option<DLLNodeID> {
        self.end_id
    }

    pub fn get_start_id(&self) -> Option<DLLNodeID> {
        self.start_id
    }
    // pub fn get_last_mut(&mut self) -> Option<&mut Option<T>> {
    //     self.end_id.map(|id| &mut self.nodes[id.0].content)
    // }

    pub fn get_content(&self, node_id: DLLNodeID) -> &T {
        if let Some(ref t_out) = self.nodes[node_id.0].content {
            t_out
        } else {
            panic!("Should not ask for already taken DLLnodeID")
        }
    }

    pub fn get_content_mut(&mut self, node_id: DLLNodeID) -> &mut T {
        if let Some(ref mut t_out) = self.nodes[node_id.0].content {
            t_out
        } else {
            panic!("Should not ask for already taken DLLnodeID")
        }
    }

    pub fn take_content_between(
        &mut self,
        bottom_node_id: DLLNodeID,
        top_node_id: DLLNodeID,
    ) -> DLLIter<'_, T, O> {
        DLLIter {
            current_id_op: self.get_next_id(bottom_node_id),
            base_arena: self,
            last_id: Some(top_node_id),
        }
    }

    pub fn take_content_above(&mut self, bottom_node_id: Option<DLLNodeID>) -> DLLIter<'_, T, O> {
        DLLIter {
            current_id_op: bottom_node_id.map_or(self.start_id, |f| Some(f)),
            base_arena: self,
            last_id: None,
        }
    }

    // fn replace_at_id(&mut self, node_id: DLLNodeID, )

    pub fn take_content_at_id(&mut self, node_id: DLLNodeID) -> T {
        let next_op = self.nodes[node_id.0].id_of_next;
        let prev_op = self.nodes[node_id.0].id_of_prev;
        let content_out = self.nodes[node_id.0].content.take();
        if let Some(prev_index) = prev_op {
            self.nodes[prev_index.0].id_of_next = next_op;
        } else {
            self.start_id = next_op;
        }
        if let Some(next_index) = next_op {
            self.nodes[next_index.0].id_of_prev = prev_op;
        } else {
            self.end_id = prev_op;
        }
        content_out.unwrap()
    }

    pub fn get_position_indicator(&self, node_id: DLLNodeID) -> &O {
        &self.nodes[node_id.0].underlying_position
    }

    pub fn get_next_id(&self, node_id: DLLNodeID) -> Option<DLLNodeID> {
        self.nodes[node_id.0].id_of_next
    }

    pub fn get_prev_id(&self, node_id: DLLNodeID) -> Option<DLLNodeID> {
        self.nodes[node_id.0].id_of_prev
    }

    pub fn replace_inside_stack_range(
        &mut self,
        bottom_node_index: DLLNodeID,
        top_node_index: DLLNodeID,
        relative_position: O,
        item: T,
    ) -> DLLNodeID {
        let id_of_this = DLLNodeID(self.nodes.len());
        self.nodes.push(DLLnode {
            underlying_position: relative_position,
            content: Some(item),
            id_of_prev: Some(bottom_node_index),
            id_of_next: Some(top_node_index),
        });
        self.nodes[bottom_node_index.0].id_of_next = Some(id_of_this);
        self.nodes[top_node_index.0].id_of_prev = Some(id_of_this);
        id_of_this
    }

    pub fn delete_stack_above_including(&mut self, node_id: DLLNodeID) {
        let prev_node_op = self.nodes[node_id.0].id_of_prev;
        if let Some(prev_node) = prev_node_op {
            self.nodes[prev_node.0].id_of_next = None;
            self.end_id = Some(prev_node);
        } else {
            self.start_id = None;
            self.end_id = None;
        }
    }

    pub fn is_taken(&mut self, node_id: DLLNodeID) -> bool {
        self.nodes[node_id.0].content.is_none()
    }

    pub fn delete(&mut self, node_id: DLLNodeID) {
        let prev_node_op = self.nodes[node_id.0].id_of_prev;
        let next_node_op = self.nodes[node_id.0].id_of_next;
        if let Some(prev_node) = prev_node_op {
            self.nodes[prev_node.0].id_of_next = next_node_op;
        } else {
            self.start_id = next_node_op;
        }
        if let Some(next_node) = next_node_op {
            self.nodes[next_node.0].id_of_prev = prev_node_op;
        } else {
            self.end_id = prev_node_op;
        }
    }

    // fn get_index_of_next(&self, index: usize) -> Option<usize> {
    //     self.dl_stack[index].index_of_next
    // }
    //
    // fn get_index_of_prev(&self, index: usize) -> Option<usize> {
    //     self.dl_stack[index].index_of_prev
    // }
}

pub struct DLLIter<'a, T, O: Ord> {
    current_id_op: Option<DLLNodeID>,
    base_arena: &'a mut ArenaDLL<T, O>,
    last_id: Option<DLLNodeID>,
}

impl<'a, T, O: Copy + Ord> Iterator for DLLIter<'a, T, O> {
    type Item = (T, O);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_id_op == self.last_id {
            None
        } else {
            let current_id = self.current_id_op?;
            let content_out = self.base_arena.take_content_at_id(current_id);
            let pos_out = *self.base_arena.get_position_indicator(current_id);
            self.current_id_op = self.base_arena.get_next_id(current_id);
            Some((content_out, pos_out))
        }
    }
}
