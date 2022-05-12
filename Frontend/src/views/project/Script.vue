<template>
  <div>
    <h3>Project: {{ pid }} | Script: {{ id }}</h3>

    <form>
      <!-- Users input -->
      <div>
        <input
          type="text"
          id="users-input"
          class="form-control"
          v-model="users"
        />
        <label for="users-input">Users</label>
      </div>
      <!-- Spawn rate input -->
      <div>
        <input
          type="text"
          id="spawn-rate-input"
          class="form-control"
          v-model="spawnRate"
        />
        <label for="spawn-rate-input">Spawn rate</label>
      </div>
      <!-- Workers input -->
      <div>
        <input
          type="text"
          id="workers-input"
          class="form-control"
          v-model="workers"
        />
        <label for="workers-input">Workers</label>
      </div>
      <div class="form-text">This will overwrite all hosts in your file</div>
      <!-- Host input -->
      <div>
        <input
          type="text"
          id="host-input"
          class="form-control"
          v-model="host"
        />
        <label class="form-label" for="host-input">Host</label>
      </div>
      <div class="form-text">
        If time is not set, the test will not stop automatically
      </div>
      <!-- Time input -->
      <div>
        <input
          type="text"
          id="time-input"
          class="form-control"
          v-model="time"
        />
        <label class="form-label" for="time-input">Time in seconds</label>
      </div>
      <div class="form-text">Descripe your test</div>
      <!-- Label input -->
      <div>
        <input
          type="text"
          id="description-input"
          class="form-control"
          v-model="description"
        />
        <label for="description-input">Description</label>
      </div>
      <!-- Submit button -->
      <button type="button" id="start-btn" @click="start">Start</button>
    </form>
    <ul>
      <li v-for="test in reversedTests" :key="test.id">
        <div>{{ test.id }}</div>
        <div>{{ test.status }}</div>
        <div>{{ test.info }}</div>
        <div>{{ test.results }}</div>
        <br />
        <div># # # # # # # # # # # # # # # # # # # # # # # # # # # # #</div>
        <br />
        <button type="button" @click="stop(test.id)">Stop</button>
        <button type="button" @click="del(test.id)">Delete</button>
      </li>
    </ul>
  </div>
</template>

<script>
export default {
  name: "Script",
  props: ["pid", "id"],
  data() {
    return {
      ws: null,
      users: null,
      spawnRate: null,
      workers: null,
      host: null,
      time: null,
      description: null,
      tests: [],
    };
  },
  computed: {
    reversedTests() {
      return this.tests.reverse();
    },
  },
  methods: {
    connenctWebsocket() {
      this.ws = new WebSocket(
        `ws://${location.host}/api/master/subscribe/${this.pid}/${this.id}`
      );
      this.ws.onopen = () => {};
      this.ws.onclose = () => {};
      this.ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        const event_type = data.event_type;
        if (event_type === "UPDATE") {
          const testsResults = data.event.tests_info;
          for (var i = 0; i < testsResults.length; i++) {
            let incomingTest = testsResults[i];
            let test = this.tests.find((test) => test.id === incomingTest.id);
            if (test) {
              test.results = incomingTest.results;
              test.status = incomingTest.status;
            }
          }
          return;
        }
        if (event_type === "TEST_STARTED") {
          const new_test = data.event.test;
          let test = this.tests.find((test) => test.id === new_test.id);
          if (!test) {
            this.tests.push(new_test);
          }
          return;
        }
        if (event_type === "TEST_DELETED") {
          const id = data.event.id;
          this.tests = this.tests.filter((test) => test.id !== id);
          return;
        }
      };
    },
    start() {
      console.log(
        this.users,
        this.spawnRate,
        this.workers,
        this.host,
        this.time,
        this.description
      );
      fetch(`/api/worker/start_test/${this.pid}/${this.id}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          users: parseInt(this.users),
          spawn_rate: parseInt(this.spawnRate),
          workers: parseInt(this.workers),
          host: this.host,
          time: parseInt(this.time),
          description: this.description,
        }),
      })
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
            let test = data.content;
            this.tests.push(test);
            this.ws.send(
              JSON.stringify({
                event_type: "TEST_STARTED",
                event: {
                  test: test,
                },
              })
            );
          } else {
            console.log(data.error);
          }
        })
        .catch(() => {});
    },
    stop(test_id) {
      fetch(`/api/master/stop_test/${this.pid}/${this.id}/${test_id}`, {
        method: "POST",
      })
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
          } else {
            console.log(data.error);
          }
        })
        .catch(() => {});
    },
    del(test_id) {
      fetch(`/api/master/delete_test/${this.pid}/${this.id}/${test_id}`, {
        method: "POST",
      })
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
            this.tests = this.tests.filter((test) => test.id !== test_id);
          } else {
            console.log(data.error);
          }
        })
        .catch(() => {});
    },
  },
  created() {
    this.connenctWebsocket();
    fetch(`/api/master/tests/${this.pid}/${this.id}`)
      .then((data) => data.json())
      .then((data) => {
        this.tests = data.content.tests;
        console.log(data.content.tests);
      })
      .catch();
  },
  unmounted() {
    this.ws.close();
  },
};
</script>

<style>
</style>