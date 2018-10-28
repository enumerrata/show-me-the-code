use std::collections::{HashMap, HashSet};

use actix::prelude::*;
use actix::spawn;
use futures::{future, Future};
use uuid::Uuid;

use super::msg::Msg;

impl actix::Message for Msg {
    type Result = ();
}

pub struct Connect(pub Uuid, pub Recipient<Msg>);

impl actix::Message for Connect {
    type Result = ();
}

pub struct Disconnect {
    pub client_id: Uuid,
    pub host_id: Option<Uuid>,
}

impl actix::Message for Disconnect {
    type Result = ();
}

type Sessions = HashMap<Uuid, Recipient<Msg>>;
type Groups = HashMap<Uuid, HashSet<Uuid>>;

pub struct SignalServer {
    sessions: Sessions,
    groups: Groups,
}

impl Default for SignalServer {
    fn default() -> SignalServer {
        SignalServer {
            sessions: HashMap::new(),
            groups: HashMap::new(),
        }
    }
}

impl SignalServer {
    fn send_to_target(&self, msg: Msg, target: &Uuid) {
        if let Some(addr) = self.sessions.get(target) {
            spawn(addr.send(msg).map_err(|_| ()));
        }
    }
}

impl Actor for SignalServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for SignalServer {
    type Result = MessageResult<Connect>;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        self.sessions.insert(msg.0, msg.1);
        MessageResult(())
    }
}

impl Handler<Disconnect> for SignalServer {
    type Result = ();

    fn handle<'a>(&mut self, msg: Disconnect, _: &mut Context<Self>) -> Self::Result {
        let groups = &mut self.groups;
        let sessions = &mut self.sessions;
        sessions.remove(&msg.client_id);
        msg.host_id
            .and_then(|host_id| groups.get_mut(&host_id))
            .into_iter()
            .flat_map(|set| {
                set.remove(&msg.client_id);
                set.iter()
            })
            .flat_map(|id| sessions.get(&id))
            .for_each(|rec| {
                spawn(
                    rec.send(Msg::Offline(msg.client_id))
                        .map(|_| {})
                        .or_else(|_| future::ok(())),
                );
            });
    }
}

impl Handler<Msg> for SignalServer {
    type Result = ();

    fn handle(&mut self, msg: Msg, _: &mut Context<Self>) -> Self::Result {
        use super::msg::{JoinRes, Msg, SendMsg};

        match msg {
            Msg::JoinRes(SendMsg {
                from: Some(from),
                to,
                content: JoinRes { ok: true, .. },
                ..
            }) => {
                self.groups.entry(from).or_default().insert(to);
                self.send_to_target(msg, &to);
            }
            _ => {}
        }
    }
}
