# EV Raster Crafts

By **EVORA**. Native Rust raster canvas engine plus an MCP bridge for Claude Desktop.

GitHub Actions builds `EV_Raster_Crafts_Setup.exe` on a Windows runner.

- Push to `main` or run the workflow manually (Actions tab > Build EV Raster Crafts installer > Run workflow), then download `EV_Raster_Crafts_Setup` from the run's Artifacts.
- Push a tag like `v0.1.0` to also publish the .exe on the Releases page.

Local build: `node build-installer.js` (needs Node 18+, Rust).
