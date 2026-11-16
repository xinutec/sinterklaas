{-
sinterklaas/gate.dhall — this repository's commit gate. The rendered
`gate.json` is committed; `the table matches its Dhall` re-renders and diffs it.
-}
let G = ../dev-lint/gate/schema.dhall

in  { name = "sinterklaas"
    , checks =
      [ G.Check::{
        , name = "formatting"
        , argv = G.inDevShell [ "cargo", "fmt", "--all", "--check" ]
        , timeout_s = 180
        }
      , G.Check::{
        , name = "clippy"
        , argv =
            G.inDevShell
              [ "cargo", "clippy", "--all-targets", "--", "-D", "warnings" ]
        , env = G.clippyTarget
        , timeout_s = 1800
        }
      , G.cargoDoc
      , {-  The whole suite against a throwaway MariaDB: without one the flow
            test skips, so the database is not optional here. Port 3320 so the
            fleet gate can run this beside fleetwatch, messages and coach.
        -}
        G.Check::{
        , name = "tests (against a real MariaDB)"
        , argv =
            G.withTestDb
              "../"
              [ "--database"
              , "sinterklaas"
              , "--user"
              , "sinterklaas"
              , "--password"
              , "sinterklaas"
              , "--port"
              , "3320"
              , "--url-env"
              , "SINTERKLAAS_TEST_DATABASE_URL"
              , "--"
              , "cargo"
              , "test"
              ]
        , timeout_s = 1800
        }
      , G.devLint "../"
      , G.checkTable "../dev-lint"
      ]
      # G.acting "../dev-lint"
    }
