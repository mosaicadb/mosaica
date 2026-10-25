# Guides

Each guide covers one task for a user who has finished a tutorial.

- [Call Mosaica from a backend](backend.md): start a session for each user, and look up values, topics and records, with the requests that answer in JSON.
- [Run Mosaica under systemd](systemd.md) on a Linux server, from the files the first tutorial made.
- [Run Mosaica with Docker](docker.md), with the database in a volume and Compose to start it.
- [Operate a deployment](operating.md): who reaches each address, memory, health, compaction and failed writes.
- [Size a deployment](sizing.md): the memory, disc and CPU a database needs, and how to estimate your own.
- [Put Mosaica behind TLS](tls.md) with nginx, and let a page on another origin use it.
- [Rebuild and replace the bundle](rebuild.md), and what happens to the `mosaica_id`s a client holds.
- [Troubleshooting](troubleshooting.md): a blank map, refused requests and a slow server, from the symptom to the cause.
