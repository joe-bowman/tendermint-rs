use gumdrop::Options;
use light_client::prelude::Height;

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Options)]
struct CliOptions {
    #[options(help = "print this help message")]
    help: bool,
    #[options(help = "enable verbose output")]
    verbose: bool,

    #[options(command)]
    command: Option<Command>,
}

#[derive(Debug, Options)]
enum Command {
    #[options(help = "run the light client and continuously sync up to the latest block")]
    Sync(SyncOpts),
}

#[derive(Debug, Options)]
struct SyncOpts {
    #[options(help = "show help for this command")]
    help: bool,
    #[options(
        help = "address of the Tendermint node to connect to",
        meta = "ADDR",
        default = "tcp://127.0.0.1:26657"
    )]
    address: tendermint::net::Address,
    #[options(
        help = "height of the initial trusted state (optional if store already initialized)",
        meta = "HEIGHT"
    )]
    trusted_height: Option<Height>,
    #[options(
        help = "path to the database folder",
        meta = "PATH",
        default = "./lightstore"
    )]
    db_path: PathBuf,
}

fn main() {
    let opts = CliOptions::parse_args_default_or_exit();
    match opts.command {
        None => {
            eprintln!("Please specify a command:");
            eprintln!("{}\n", CliOptions::command_list().unwrap());
            eprintln!("{}\n", CliOptions::usage());
            std::process::exit(1);
        }
        Some(Command::Sync(sync_opts)) => sync_cmd(sync_opts),
    }
}

fn sync_cmd(opts: SyncOpts) {
    use light_client::components::scheduler;
    use light_client::prelude::*;

    let primary_addr = opts.address;
    let primary: PeerId = "BADFADAD0BEFEEDC0C0ADEADBEEFC0FFEEFACADE".parse().unwrap();

    let mut peer_map = HashMap::new();
    peer_map.insert(primary, primary_addr);

    let mut io = ProdIo::new(peer_map);

    let db = sled::open(opts.db_path).unwrap_or_else(|e| {
        println!("[ error ] could not open database: {}", e);
        std::process::exit(1);
    });

    let mut light_store = SledStore::new(db);

    if let Some(height) = opts.trusted_height {
        let trusted_state = io.fetch_light_block(primary, height).unwrap_or_else(|e| {
            println!("[ error ] could not retrieve trusted header: {}", e);
            std::process::exit(1);
        });

        light_store.insert(trusted_state, VerifiedStatus::Verified);
    }

    let peers = Peers {
        primary,
        witnesses: Vec::new(),
    };

    let state = State {
        peers,
        light_store: Box::new(light_store),
        verification_trace: HashMap::new(),
    };

    let options = Options {
        trust_threshold: TrustThreshold {
            numerator: 1,
            denominator: 3,
        },
        trusting_period: Duration::from_secs(36000),
        clock_drift: Duration::from_secs(1),
        now: Time::now(),
    };

    let predicates = ProdPredicates;
    let voting_power_calculator = ProdVotingPowerCalculator;
    let commit_validator = ProdCommitValidator;
    let header_hasher = ProdHeaderHasher;

    let verifier = ProdVerifier::new(
        predicates,
        voting_power_calculator,
        commit_validator,
        header_hasher,
    );

    let clock = SystemClock;
    let scheduler = scheduler::schedule;
    let fork_detector = RealForkDetector::new(header_hasher);

    let mut light_client = LightClient::new(
        state,
        options,
        clock,
        scheduler,
        verifier,
        fork_detector,
        io,
    );

    loop {
        match light_client.verify_to_highest() {
            Ok(light_block) => {
                println!("[ info  ] synced to block {}", light_block.height());
            }
            Err(e) => {
                println!("[ error ] sync failed: {}", e);
            }
        }
        std::thread::sleep(Duration::from_millis(800));
    }
}
