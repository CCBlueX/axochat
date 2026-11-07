use super::{now_ms, Author, Channel, ClientPacket};

use std::collections::VecDeque;
use uuid::Uuid;

const CAPACITY: usize = 1000;

/// The recent messages of this server run; ids count up from 1 with every start.
pub struct History {
    messages: VecDeque<Recorded>,
    next_id: u64,
}

pub struct Recorded {
    id: u64,
    time: i64,
    /// The id in the database.
    author: Uuid,
    audience: Audience,
    content: String,
}

#[derive(Clone, Copy)]
pub enum Audience {
    Global,
    Direct { receiver: Uuid },
}

impl History {
    pub fn new() -> History {
        History {
            messages: VecDeque::with_capacity(CAPACITY),
            next_id: 1,
        }
    }

    pub fn record(&mut self, author: Uuid, audience: Audience, content: String) -> &Recorded {
        if self.messages.len() == CAPACITY {
            self.messages.pop_front();
        }
        self.messages.push_back(Recorded {
            id: self.next_id,
            time: now_ms(),
            author,
            audience,
            content,
        });
        self.next_id += 1;
        self.messages.back().expect("just recorded")
    }
}

impl Recorded {
    /// A direct message shows each side the other one.
    fn channel(&self, viewer: Option<Uuid>) -> Channel {
        match self.audience {
            Audience::Global => Channel::Global,
            Audience::Direct { receiver } if viewer == Some(self.author) => Channel::Direct { user: receiver },
            Audience::Direct { .. } => Channel::Direct { user: self.author },
        }
    }

    pub fn packet(&self, viewer: Option<Uuid>, author: &Author) -> ClientPacket {
        ClientPacket::ChatMessage {
            channel: self.channel(viewer),
            id: self.id,
            time: self.time,
            author: author.clone(),
            content: self.content.clone(),
        }
    }
}
