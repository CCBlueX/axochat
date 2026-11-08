use super::{now_ms, Author, Channel, ClientPacket};

use std::collections::VecDeque;
use uuid::Uuid;

const CAPACITY: usize = 1000;

/// The recent messages of this server run; ids start at the current time in milliseconds,
/// so they keep growing across restarts.
pub struct History {
    messages: VecDeque<Recorded>,
    next_id: u64,
}

pub struct Recorded {
    pub id: u64,
    time: i64,
    /// The id in the database.
    pub author: Uuid,
    pub audience: Audience,
    /// What was delivered.
    pub content: String,
    /// What the author wrote, if rustrict censored it; only staff see it.
    pub original: Option<String>,
    pub removed: bool,
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
            next_id: now_ms() as u64,
        }
    }

    pub fn record(&mut self, author: Uuid, audience: Audience, content: String, original: Option<String>) -> &Recorded {
        if self.messages.len() == CAPACITY {
            self.messages.pop_front();
        }
        self.messages.push_back(Recorded {
            id: self.next_id,
            time: now_ms(),
            author,
            audience,
            content,
            original,
            removed: false,
        });
        self.next_id += 1;
        self.messages.back().expect("just recorded")
    }

    pub fn get(&self, id: u64) -> Option<&Recorded> {
        self.messages.get(self.index(id)?)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Recorded> {
        let index = self.index(id)?;
        self.messages.get_mut(index)
    }

    fn index(&self, id: u64) -> Option<usize> {
        let first = self.messages.front()?.id;
        usize::try_from(id.checked_sub(first)?).ok()
    }
}

impl Audience {
    pub fn reaches(&self, user: Uuid) -> bool {
        match self {
            Audience::Global => true,
            Audience::Direct { receiver } => *receiver == user,
        }
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

    pub fn packet(&self, viewer: Option<Uuid>, author: &Author, staff: bool) -> ClientPacket {
        ClientPacket::ChatMessage {
            channel: self.channel(viewer),
            id: self.id,
            time: self.time,
            author: author.clone(),
            content: self.content.clone(),
            original: staff.then(|| self.original.clone()).flatten(),
        }
    }
}
