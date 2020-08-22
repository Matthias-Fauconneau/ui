#![feature(once_cell)]
fn main() { std::lazy::SyncLazy::force(&ui::edit::COMPOSE); }
