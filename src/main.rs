use std::{collections::{HashMap, HashSet}, sync::Arc};
use serenity::{
    async_trait,
    client::bridge::gateway::{ShardId, ShardManager},
    framework::standard::{
        Args, CommandResult,
        DispatchError, StandardFramework,
        macros::{command, group, help, hook},
    },
    http::Http,
    model::{
        channel::{Message},
        gateway::Ready,
    },
};

use serenity::prelude::*;
use tokio::sync::Mutex;

use rand::{seq::IteratorRandom, thread_rng};

// A container type is created for inserting into the Client's `data`, which
// allows for data to be accessible across all events and framework commands, or
// anywhere else that has a copy of the `data` Arc.
struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct CommandCounter;

impl TypeMapKey for CommandCounter {
    type Value = HashMap<String, u64>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[description = "Available booru website: rule34, hypnohub, konachan, realbooru, xbooru, yandere"]
#[summary = "For booru lover's"]
#[commands(ochako, ochako_latency, ochako_about)]
struct General;

#[help]
#[individual_command_tip = "Hi Senpai :heart:"]
#[command_not_found_text = "Could not find: `{}`."]
// #[max_levenshtein_distance(10)]
// #[indention_prefix = "+"]
// #[lacking_permissions = "Hide"]
// #[lacking_role = "Nothing"]
// #[wrong_channel = "Strike"]
async fn my_help(context: &Context, msg: &Message) -> CommandResult {
    let _ = msg.channel_id.say(context, "\n**Hi Senpai :heart:**\n\n*Available booru website:* \
    \nrule34,\nhypnohub,\nkonachan,\nrealbooru,\nxbooru,\nyandere\n\n*For use the ochako command type:*\n\
    --ochako {site} {optional: tag}").await;
    Ok(())
}

#[hook]
async fn after(_ctx: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{}'", command_name),
        Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
    }
}

#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    println!("Could not find command named '{}'", unknown_command_name);
}

#[hook]
async fn delay_action(ctx: &Context, msg: &Message) {
    // You may want to handle a Discord rate limit if this fails.
    let _ = msg.react(ctx, '⏱').await;
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    if let DispatchError::Ratelimited(info) = error {

        // We notify them only once.
        if info.is_first_try {
            let _ = msg
                .channel_id
                .say(&ctx.http, &format!("Try this again in {} seconds.", info.as_secs()))
                .await;
        }
    }
}

use serenity::{futures::future::BoxFuture, FutureExt};

fn _dispatch_error_no_macro<'fut>(ctx: &'fut mut Context, msg: &'fut Message, error: DispatchError) -> BoxFuture<'fut, ()> {
    async move {
        if let DispatchError::Ratelimited(info) = error {

            if info.is_first_try {
                let _ = msg
                    .channel_id
                    .say(&ctx.http, &format!("Try this again in {} seconds.", info.as_secs()))
                    .await;
            }
        };
    }.boxed()
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token.
    let token = "";
    let http = Http::new_with_token(&token);

    // We will fetch your bot's owners and id
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        },
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| c
            .with_whitespace(true)
            .on_mention(Some(bot_id))
            .prefix("--")
            // In this case, if "," would be first, a message would never
            // be delimited at ", ", forcing you to trim your arguments if you
            // want to avoid whitespaces at the start of each.
            .delimiters(vec![", ", ","])
            // Sets the bot's owners. These will be used for commands that
            // are owners only.
            .owners(owners))
        .after(after)
        .unrecognised_command(unknown_command)
        .on_dispatch_error(dispatch_error)
        .help(&MY_HELP)
        .group(&GENERAL_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<CommandCounter>(HashMap::default());
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

#[command]
async fn ochako_about(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, "A booru bot created by Pyro#0239 :heart:").await?;

    Ok(())
}

#[command]
async fn ochako_latency(ctx: &Context, msg: &Message) -> CommandResult {
    // The shard manager is an interface for mutating, stopping, restarting, and
    // retrieving information about shards.
    let data = ctx.data.read().await;

    let shard_manager = match data.get::<ShardManagerContainer>() {
        Some(v) => v,
        None => {
            msg.reply(ctx, "There was a problem getting the shard manager").await?;

            return Ok(());
        },
    };

    let manager = shard_manager.lock().await;
    let runners = manager.runners.lock().await;

    // Shards are backed by a "shard runner" responsible for processing events
    // over the shard, so we'll get the information about the shard runner for
    // the shard this command was sent over.
    let runner = match runners.get(&ShardId(ctx.shard_id)) {
        Some(runner) => runner,
        None => {
            msg.reply(ctx,  "No shard found").await?;

            return Ok(());
        },
    };

    msg.reply(ctx, &format!("The shard latency is {:?}", runner.latency)).await?;

    Ok(())
}

fn random_url(v: Vec<&str>) -> &str {
    let mut rng = thread_rng();
    let sample = v.iter().choose_multiple(&mut rng, 1);
    let test = &sample.to_owned();

    test[0].to_owned()
}

#[command]
async fn ochako(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args: Vec<&str> = args.rest().split(" ").collect();
    let mut request = "".to_owned();
    let mut name = "";
    let mut page = "";
    match args[0] {

        "rule34" => {
            name = "rule34.xxx";
            page = "index.php";
        },
        "hypnohub" => {
            name = "hypnohub.net";
            page = "post.xml";
        },
        "konachan" => {
            name = "konachan.com";
            page = "post.xml";
        },
        "realbooru" => {
            name = "realbooru.com";
            page = "index.php";
        },
        "xbooru" => {
            name = "xbooru.com";
            page = "index.php";
        },
        "yandere" => {
            name = "yande.re";
            page = "post.xml";
        }

        _ => {
            msg.reply_mention(&ctx.http, "You need enter valid arguments, type help for more informations.").await?;
        }
    }
    if args.len() == 1 {
        request = format!("https://{name}/{page}?page=dapi&s=post&q=index&tags=&limit=100", name = name, page = page)
    }
    else if args.len() == 2 {
        request = format!("https://{name}/{page}?page=dapi&s=post&q=index&tags={tags}&limit=100", name = name, page = page, tags = args[1])
    }
    let body = reqwest::get(request.as_str())
        .await?;
    let body = body.text().await?;

    let splited: Vec<_> = body.as_str().split("file_url=\"").collect();

    let mut list_of_url = Vec::new();
    for e in splited {
        let url: Vec<_> = e.split("\"").collect();
        let url = url[0];
        list_of_url.push(url);
    }
    list_of_url.remove(0);
    let final_url = random_url(list_of_url);
    msg.reply(&ctx.http, &format!("{}", final_url)).await?;
    Ok(())
}