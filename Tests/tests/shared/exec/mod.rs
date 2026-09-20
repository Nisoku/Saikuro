use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    channels::register(suite);
    join::register(suite);
    select::register(suite);
    time::register(suite);
}

pub mod channels;
pub mod join;
pub mod select;
pub mod time;
