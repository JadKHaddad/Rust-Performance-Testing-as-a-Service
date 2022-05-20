<template>
  <div>
    <nav class="uk-navbar-container uk-margin" uk-navbar>
      <div class="uk-navbar-left">
        <ul class="uk-navbar-nav">
          <li><a href="#">Item</a></li>
          <li><a href="#">Item</a></li>
          <li><a href="#">Item</a></li>
        </ul>
      </div>
      <div class="uk-navbar-right">
        <ul class="uk-navbar-nav">
          <li>
            <div class="uk-navbar-item">
              <span uk-icon="users"></span>
              <label>{{ connected_clients_count }}</label>
            </div>
          </li>
          <li>
            <div class="uk-navbar-item">
              <span uk-icon="play"></span>
              <label>{{ running_tests_count }}</label>
            </div>
          </li>
        </ul>
      </div>
    </nav>
    <div class="content">
    <router-view />
    </div>
  </div>
</template>

<script>
export default {
  name: "App",
  data() {
    return {
      ws: null,
      connected_clients_count: 0,
      running_tests_count: 0,
    };
  },
  methods: {
    connenctWebsocket() {
      this.ws = new WebSocket(`ws://${location.host}/api/master/ws`);
      this.ws.onopen = () => {};
      this.ws.onclose = () => {};
      this.ws.onmessage = (event) => {
        console.log(event.data);
        const data = JSON.parse(event.data);
        const event_type = data.event_type;
        if (event_type === "INFORMATION") {
          this.connected_clients_count = data.event.connected_clients_count;
          this.running_tests_count = data.event.running_tests_count;
        }
      };
    },
  },
  created() {
    this.connenctWebsocket();
  },
};
</script>

<style>
</style>
