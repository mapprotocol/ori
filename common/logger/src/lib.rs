// Copyright 2021 MAP Protocol Authors.
// This file is part of MAP Protocol.

// MAP Protocol is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// MAP Protocol is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with MAP Protocol.  If not, see <http://www.gnu.org/licenses/>.

use env_logger::{Builder, Target, Env};
#[allow(unused_imports)]
use log::LevelFilter;

pub struct LogConfig {
    pub filter: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            filter: "info".into(),
        }
    }
}

pub fn init(config: LogConfig) {
    let env = Env::default()
        .filter_or("RUST_LOG", config.filter)
        .write_style_or("RUST_LOG_STYLE", "never");

    let mut builder = Builder::from_env(env);
    builder.target(Target::Stdout);
    // builder.filter(None, LevelFilter::Info);
    // builder.write_style(WriteStyle::Never);
    builder.format_module_path(false);
    builder.init();
}
