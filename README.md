# About

A DVM client for sourcing notes from the MN nostr community

## Setup

This service will keep track of npubs to follow locally, allow DMs from the specified "admin" to add/remove npubs from the list, and supply the latest kind1 notes from the list of npubs when requested

## Admin Tools

Commands sent via DM are processed if the npub is an admin. Available commands:

- `add npub [npub]` - adds the npub to the local list
- `remove npub [npub]` - removes the npubs from the local list
- `list npubs` - lists all npubs on the list
- `add relay [relay]` - adds a read-relay
- `remove relay [relay]` - removes a relay
- `list relays` - lists all relays used by the DVM
