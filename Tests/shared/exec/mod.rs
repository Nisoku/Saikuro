use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    channels::register(suite);
    select::register(suite);
}

pub mod channels;
pub mod select;
