<template>
  <div>
    <h2>connected clients: {{ connected_clients_count }} | running_tests_count:
    {{ running_tests_count }}
    </h2>
    <router-view />
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
      this.ws.onopen = () => {
      };
      this.ws.onclose = () => {
        
      };
      this.ws.onmessage = (event) => {
        console.log(event.data);
        const data = JSON.parse(event.data);
        const event_type = data.event_type;
        if (event_type === "INFORMATION"){
          this.connected_clients_count = data.event.connected_clients_count;
          this.running_tests_count = data.event.running_tests_count;
        }
      };
    },
  },
  created() {
    this.connenctWebsocket();
  }
};
</script>

<style>
</style>
