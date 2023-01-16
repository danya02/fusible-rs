# fusible-rs
Wrapper around the FUSE API for making simple ad-hoc virtual filesystems.

This is not intended as a general-purpose FUSE library.
Instead, this is intended to be a simple way to make a virtual filesystem
for a specific purpose.
In particular, this will probably never have a stable API.

The FUSE API is complex, and this library is intended to facilitate
the relatively simple use case of having "a file backed by a script" (i.e. the `/proc` filesystem).

Examples of what this could be useful for:
- viewing a single large file as a directory of smaller files
- viewing a directory of files as a single large file
- (todo)

## Known limitations

- All directories and files have permissive attributes (i.e. `0o777`).