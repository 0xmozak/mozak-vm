mod transaction;

/// The server that is responsible for handling the requests from the client.
struct Server();

impl Server {
    /// Creates a new server.
    fn new() -> Self { Self() }

    /// Starts the server.
    fn start(&self) {
        println!("Server started.");
    }

    fn stop(&self) {
        println!("Server stopped.");
    }

    fn get_next_transaction(&self) -> Transaction {
        // TODO - implement properly
        Transaction::random();
    }
}
