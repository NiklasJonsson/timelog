# TODO

1. Dependency changes:
    * `dirs` crate for home dir
    * `clap` for argument parsing
2. Upgrade all deps.
3. Make into a CLI framework.
    * All functions take a Cli context which handles output, global object storage etc.
    * Use proc macros to derive the interface from function signature.
    * Commands return a result that the CLI framework prints and exits with exit code approp.
    * Command registration is done in main()
4. Move command implementation into cli.rs.
5. Create separate integration tests.
6. Print format of log file at the top.
7. archive command to save the current file?
8. Better error messages for invalid format.
