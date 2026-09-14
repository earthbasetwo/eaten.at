//! eaten.at web application, as a library so integration tests can
//! build the router with mock backends.

pub mod app;
pub mod auth;
pub mod bsky;
pub mod cache;
pub mod db;
pub mod editor;
pub mod error;
pub mod feed;
pub mod hosting;
pub mod img;
pub mod labels;
pub mod model;
pub mod paths;
pub mod places;
pub mod publish;
pub mod read;
pub mod routes;
pub mod search;
pub mod security;
pub mod settings;
pub mod state;
pub mod tags;
pub mod view;
