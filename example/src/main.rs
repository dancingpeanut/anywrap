mod error;
mod app;

fn main() {
    println!("Macro error: {:?}", anyerr!("hello {}", "anywrap"));
    if let Err(e) = app::wrap_to_io() {
        println!("--11: {e:?}");
        if let error::Error::Any { source, .. } = e {
            if let Some(e) = source.downcast_ref::<std::io::Error>() {
                println!("IO error: {:?}", e);
            } else {
                println!("Unknown error {source:?}");
            }
        }
    }
    if let Err(e) = app::auto() {
        println!("--12: {e:?}");
    }
    if let Err(e) = app::with_context() {
        println!("--13: {e:?}");
    }
    if let Err(e) = app::with_chain() {
        println!("--15 display: {e}\n\n\n\n");
        println!("--15 debug: {e:?}");
    }
}
