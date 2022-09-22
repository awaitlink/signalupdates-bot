# `signalupdates-bot`

An [unofficial](#disclaimer) bot that creates posts about new tags in Signal repositories in the corresponding [beta feedback](https://community.signalusers.org/c/25) topics.

## Running an instance

To run this bot, do the following:

### Setup

1. Install [Rust](https://www.rust-lang.org/tools/install).
1. Follow the Cloudflare Workers [Get started guide](https://developers.cloudflare.com/workers/get-started/guide/) from the beginning up to, but not including, step 3.
1. Clone this repository using [Git](https://www.git-scm.com):
    ```shell
    git clone https://github.com/u32i64/signalupdates-bot
    ```

### Configuration
#### General

1. Copy the [`wrangler.example.toml`](/wrangler.example.toml) file to `wrangler.toml`.
1. In `wrangler.toml`, fill in the following:

    Field | Description
    ---|---
    `account_id` | Available in the [Cloudflare dashboard](https://dash.cloudflare.com/?to=/:account/workers/overview).
    `id` for `STATE` binding in `kv_namespaces` | Create a KV namespace [in the dashboard](https://dash.cloudflare.com/?to=/:account/workers/kv/namespaces), then copy its ID. If you'll be deploying the `staging` variant as well, it is recommended to create a separate KV namespace for it.
    `USER_ID` | The bot's numeric user ID like `12345`. You can find it by inspecting the HTML or JSON of any of the bot's posts. It is used to find the bot's posts when confirming post approval (which is done without the API key in case the post is returned even if it's not approved, so the `yours` property of posts can't be used).
    `TOPIC_ID_OVERRIDE` | If you'd like all of the bot's posts to go to a single topic, set this variable to the topic's ID, for example `12345`. Otherwise, leave it empty.
    `DRY_RUN` | If you'd like the bot to skip actually posting to Discourse, but otherwise do everything else, including modifying the state (with dummy post numbers), set this to `true`. Otherwise, leave it empty.

1. In the KV namespace(s) you created, manually create a key-value pair with the key `state` and a value like:

    ```json
    {
        "android": {
            "last_posted_tag_previous_release": { "name": "v1.2.3" },
            "last_posted_tag": { "name": "v1.3.0" }
        },
        "ios": {
            "last_posted_tag_previous_release": { "name": "1.2.0.4-beta" },
            "last_posted_tag": { "name": "1.3.0.4-beta" }
        },
        "desktop": {
            "last_posted_tag_previous_release": { "name": "v1.2.0-beta.1" },
            "last_posted_tag": { "name": "v1.3.0-beta.1" }
        }
    }
    ```

    Adjust the value accordingly if you'd like to skip posting many old versions after starting the bot.

    You can also add other values used in `PlatformState` (see [`src/state.rs`](/src/state.rs)), but this is not required, as default values will be used automatically.

#### Discourse

Configure the bot's access to Discourse.

1. Create a new user for the bot in the Discourse instance, if necessary.
1. Log in as that user and create a Discourse user API key, for example using this CLI tool: [KengoTODA/discourse-api-key-generator](https://github.com/KengoTODA/discourse-api-key-generator).

    To use this tool, you will need to install [NPM](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm), specifically you need the `npx` command from it.
1. In the folder where you have the bot's code, run:

    ```shell
    wrangler secret put DISCOURSE_API_KEY -e production
    ```

    When prompted, paste the API key you've created and press <kbd>Enter</kbd>.

    If you'll be deploying the `staging` variant as well, run the same command with `staging` instead of `production` (and also do this for future commands below). Note that you might not be able to create two different API keys using the tool above (when you create a new one, the old one may be removed); you may have to use the same API key for both `production` and `staging`.

    **Note:** you may have to publish the bot once before running this command. To publish it, see the [**Deploying**](#deploying) section below.

#### Discord

To send the bot's log to a Discord channel in case an error occurs, set up a webhook.

1. Create a webhook in a Discord server (guild) by following [this article](https://support.discord.com/hc/en-us/articles/228383668-Intro-to-Webhooks).
1. Once you have the webhook URL, configure the bot with it, similar to what you did with the Discourse API key:

    ```shell
    wrangler secret put DISCORD_WEBHOOK_URL -e production
    ```

### Deployment

Run the following command to deploy the bot:

```shell
wrangler publish -e production
```

The `production` variant is configured by default to run every 10 minutes. For the `staging` variant, you have to invoke it manually by visiting its URL (that looks like `signalupdates-bot-staging.<your-workers-subdomain>.workers.dev`).

## Acknowledgements

Special thanks to all participants of the discussion in [this topic](https://community.signalusers.org/t/42818) and to `@newuser` for developing their version of the bot.

## Disclaimer
This is an unofficial project. It is *not* affiliated with the Signal Technology Foundation or Signal Messenger, LLC.
