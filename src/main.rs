use api::{City, House};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;
use teloxide::{
    adaptors::Throttle, adaptors::throttle::Limits, prelude::*, utils::command::BotCommands,
};
use tokio::sync::{Mutex, mpsc, mpsc::Receiver};

mod api;

trait LogErr {
    fn log_err(&self);
}

impl<T, E: std::fmt::Display> LogErr for Result<T, E> {
    fn log_err(&self) {
        if let Err(e) = self {
            log::error!("An error has occurred: {}", e);
        }
    }
}

fn holland2stay_url() -> &'static str {
    "https://holland2stay.com/residences?available_to_book%5Bfilter%5D=Available+to+book%2C179&page=1"
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Display this text.")]
    Help,

    #[command(description = "Subscribe to a city")]
    Watch(City),

    #[command(description = "Unscubscribe from a city")]
    Unwatch(City),

    #[command(description = "Unscubscribe from all cities")]
    Unsubscribe,

    #[command(description = "List subscriptions")]
    Subscriptions,
}

type ObserverMutex = Arc<Mutex<HashMap<ChatId, HashSet<City>>>>;
type HousesMutex = Arc<Mutex<HashSet<House>>>;

async fn answer<B: Requester>(
    bot: B,
    msg: Message,
    cmd: Command,
    observers_mutex: ObserverMutex,
    houses: HousesMutex,
) -> Result<(), B::Err> {
    let chat_id = msg.chat.id;

    match cmd {
        Command::Help => {
            bot.send_message(chat_id, Command::descriptions().to_string())
                .await?;
        }
        Command::Watch(city) => {
            observers_mutex
                .lock()
                .await
                .entry(chat_id)
                .or_default()
                .insert(city);
            bot.send_message(
                chat_id,
                format!("You are now subscribed to houses in {}.", city),
            )
            .await?;

            let houses = houses.lock().await;
            let mut send_url = false;
            for house in houses.iter().filter(|house| house.city == city) {
                bot.send_message(chat_id, format!("There is this house: {}", house))
                    .await?;
                send_url = true;
            }
            if send_url {
                bot.send_message(chat_id, holland2stay_url()).await?;
            }
        }
        Command::Unwatch(city) => {
            if observers_mutex
                .lock()
                .await
                .entry(chat_id)
                .or_default()
                .remove(&city)
            {
                bot.send_message(
                    chat_id,
                    format!("You are now unsubscribed from houses in {}.", city),
                )
                .await?;
            } else {
                bot.send_message(
                    chat_id,
                    format!("You were already unsubscribed from houses in {}.", city),
                )
                .await?;
            }
        }
        Command::Unsubscribe => {
            if let Some(cities) = observers_mutex.lock().await.remove(&chat_id) {
                let cities_list = itertools::join(cities, ", ");
                bot.send_message(
                    chat_id,
                    format!("You are now unsubscribed from {}.", cities_list),
                )
                .await?;
            } else {
                bot.send_message(chat_id, "You were already unsubscribed.")
                    .await?;
            }
        }
        Command::Subscriptions => {
            if let Some(cities) = observers_mutex.lock().await.get(&chat_id) {
                let cities_list = itertools::join(cities, ", ");
                bot.send_message(chat_id, format!("You are subscribed to {}.", cities_list))
                    .await?;
            } else {
                bot.send_message(chat_id, "You have no subscriptions.")
                    .await?;
            }
        }
    };

    Ok(())
}

async fn get_houses_and_notify<Bot: Requester>(
    observers_mutex: &ObserverMutex,
    bot: &mut Bot,
    old_houses: &HashSet<House>,
) -> Option<HashSet<House>> {
    let observers = observers_mutex.lock().await;
    if observers.is_empty() {
        log::info!("no observers, going to sleep until woken up");
        return None;
    }

    let all_cities: HashSet<City> =
        observers
            .iter()
            .fold(HashSet::new(), |mut acc, (_, cities)| {
                acc.extend(cities);
                acc
            });

    match api::query_houses_in_cities(all_cities.iter()).await {
        Ok(new_houses) => {
            let new_houses: HashSet<House> = HashSet::from_iter(new_houses.into_iter());
            let mut send_url = HashSet::<ChatId>::new();
            for house in new_houses.difference(&old_houses) {
                let observers = observers
                    .iter()
                    .filter(|(_, cities)| cities.contains(&house.city));
                for (&chat_id, _) in observers {
                    bot.send_message(chat_id, format!("I found a new house! {}", house))
                        .await
                        .log_err();
                    send_url.insert(chat_id);
                }
            }
            for chat_id in send_url {
                bot.send_message(chat_id, holland2stay_url())
                    .await
                    .log_err();
            }
            Some(new_houses)
        }
        Err(err) => {
            for (&chat_id, _) in observers.iter() {
                bot.send_message(
                    chat_id,
                    "An error occurred while fetching houses from holland2stay.",
                )
                .await
                .log_err();
            }
            log::error!(
                "An error occurred while fetching houses from holland2stay: {}",
                err
            );
            None
        }
    }
}

fn setup_periodic_check_timer(period: std::time::Duration) -> Receiver<()> {
    let (timer_tx, timer_rx) = mpsc::channel(2);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(period).await;
            if let Err(e) = timer_tx.send(()).await {
                log::error!("Error sending timer message: {}", e);
            }
        }
    });
    timer_rx
}

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, log::LevelFilter::Info)
        .init();
    dotenv::dotenv().ok();

    let token = std::env::var("TOKEN").expect("TOKEN environment variable not found");
    let bot = Bot::new(token).throttle(Limits::default());

    let mut on_check_houses = setup_periodic_check_timer(std::time::Duration::from_secs(15));

    let observers: ObserverMutex = Arc::new(Mutex::new(HashMap::new()));
    let houses_mutex: HousesMutex = Arc::new(Mutex::new(HashSet::new()));

    let observers_clone = observers.clone();
    let houses_clone = houses_mutex.clone();
    let mut bot_clone = bot.clone();
    tokio::spawn(async move {
        loop {
            {
                let mut houses = houses_clone.lock().await;
                if let Some(new_houses) =
                    get_houses_and_notify(&observers_clone, &mut bot_clone, &houses).await
                {
                    *houses = new_houses;
                }
            }

            let now = std::time::Instant::now();
            while let None = on_check_houses.recv().await {}
            let slept_for = std::time::Instant::now().duration_since(now);
            log::info!("Awake! slept for {:.2}s", slept_for.as_secs_f64());
        }
    });

    Command::repl(
        bot,
        move |bot: Throttle<Bot>, msg: Message, cmd: Command| {
            answer(bot, msg, cmd, observers.clone(), houses_mutex.clone())
        },
    )
    .await;
}
