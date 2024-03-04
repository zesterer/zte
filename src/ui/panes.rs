use super::*;

pub struct Panes;

impl Element for Panes {
    fn handle(&mut self, event: Event) -> Result<Resp, Event> {
        match event.to_action(|e| None) {
            //Some(_) => todo!(),
            _ => Err(event),
        }
    }
}
