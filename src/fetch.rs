use std::io;

/// Result of one HTTP GET. A 404 is an ordinary outcome, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    Body(Vec<u8>),
    NotFound,
}

/// The only way the launcher talks to the network.
pub trait Fetch {
    fn get(&self, url: &str) -> io::Result<Fetched>;
}

/// The real network adapter. Sends the age-check cookie and Japanese
/// `Accept-Language` that the maniax-touch product pages expect.
pub struct HttpFetch;

impl Fetch for HttpFetch {
    fn get(&self, url: &str) -> io::Result<Fetched> {
        let response = ureq::get(url)
            .header("Cookie", "adultchecked=1")
            .header("Accept-Language", "ja-JP,ja;q=0.9")
            .call();
        match response {
            Ok(mut response) => {
                let body = response
                    .body_mut()
                    .read_to_vec()
                    .map_err(io::Error::other)?;
                Ok(Fetched::Body(body))
            }
            Err(ureq::Error::StatusCode(404)) => Ok(Fetched::NotFound),
            Err(err) => Err(io::Error::other(err)),
        }
    }
}
