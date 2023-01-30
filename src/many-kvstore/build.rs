use vergen::{vergen, Config};

fn main() {
    let mut config = Config::default();
    *config.git_mut().enabled_mut() = true;
    vergen(config).expect("Vergen could not run.")
}
