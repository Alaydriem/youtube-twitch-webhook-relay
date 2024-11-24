![YouTube Channel Subscribers](https://img.shields.io/youtube/channel/subscribers/UCXgqRZv7bHsKzwYBrtA9DFA?label=Youtube%20Subscribers&logo=Alaydriem&style=flat-square)

<div align="center">

  <h1>Youtube + Twitch Webhook Relay for Discord</h1>

<a href="https://www.youtube.com/@Alaydriem"><img src="https://raw.githubusercontent.com/alaydriem/bedrock-material-list/master/docs/subscribe.png" width="140"/></a>

<a href="https://discord.gg/CdtchD5zxr">Connect on Discord</a>

  <p>
    <strong>Pulls events from Youtube Playlist RSS Feeds, and Twitch Schedules to broadcast to Discord channels</strong>
  </p>
  <hr />
</div>

### Features

- Announce new Youtube videos to Discord (both channels and forums)

```yaml
log_level: info
playlist:
  - id: <YOUR_YT_PLAYLIST_ID>
    name: "name"
    webhooks:
      - destination: discord
        is_forum: false
        groups:
          - "<@&DiscordNotificationRoleId>"
        urls:
          - https://discord.com/api/webhooks/.../...
      - destination: bluesky
        credentials:
          username: alaydriem.com
          password: <bluesky_app_password>
```
