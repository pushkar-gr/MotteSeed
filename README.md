# Rust BitTorrent Client

A BitTorrent client implementation in Rust, focusing on performance, correctness, and modularity.

## Features

* Parsing `.torrent` files with support for optional fields.
* Tracker communication (HTTP and UDP) with optional protocol fields.
* Peer connection and communication.
* Piece management and verification.
* Disk I/O for file storage.

### Supported Optional Fields

**Torrent File Fields:**
- `announce-list`: Backup tracker URLs
- `comment`: Torrent description
- `created by`: Creator program information
- `creation date`: Unix timestamp of creation
- `encoding`: String encoding format

**Info Dictionary Fields:**
- `private`: DHT/peer exchange control flag
- `source`: Source identification for private torrents

**Tracker Communication Fields:**
- Request: `event`, `numwant`, `key`, `trackerid`
- Response: `min_interval`, `tracker_id`, `complete`, `incomplete`, `warning_message`

## Getting Started

1.  **Clone the repository:**

    ```bash
    git clone https://github.com/pushkar-gr/MotteSeed.git
    cd MotteSeed
    ```

2.  **Build the project:**

    ```bash
    cargo build --release
    ```

3.  **Run the client:**

    ```bash
    cargo run --release -- <torrent_file_path>
    ```

## Contributing

Contributions are welcome! Please submit pull requests or open issues for bugs and feature requests.
