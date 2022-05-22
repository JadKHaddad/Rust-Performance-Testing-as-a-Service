<template>
  <div>
    <button
      class="uk-button uk-button-default uk-margin-small-right"
      type="button"
      uk-toggle="target: #start-modal"
    >
      Start
    </button>

    <div id="start-modal" uk-modal ref="start-modal">
      <div class="uk-modal-dialog uk-modal-body">
        <form>
          <div class="uk-margin">
            <input
              class="uk-input"
              type="text"
              placeholder="Users"
              v-model="users"
            />
          </div>
          <div class="uk-margin">
            <input
              class="uk-input"
              type="text"
              placeholder="Spawn rate"
              v-model="spawnRate"
            />
          </div>
          <div class="uk-margin">
            <input
              class="uk-input"
              type="text"
              placeholder="Workers"
              v-model="workers"
            />
          </div>
          <div class="uk-margin">
            <label class="uk-form-label" for="host"
              >This will overwrite all hosts in your file</label
            >
            <input
              id="host"
              class="uk-input"
              type="text"
              placeholder="Host"
              v-model="host"
            />
          </div>
          <div class="uk-margin">
            <label class="uk-form-label" for="time"
              >If time is not set, the test will not stop automatically</label
            >
            <input
              id="time"
              class="uk-input"
              type="text"
              placeholder="Time is seconds"
              v-model="time"
            />
          </div>
          <div class="uk-margin">
            <input
              class="uk-input"
              type="text"
              placeholder="Description"
              v-model="description"
            />
          </div>
          <button
            class="uk-button uk-button-default uk-margin-small-right"
            type="button"
            @click="start"
          >
            Start
          </button>
        </form>
      </div>
    </div>

    <h3>Project: {{ pid }} | Script: {{ id }}</h3>
    <!--
    <form>

      <div>
        <input
          type="text"
          id="users-input"
          class="form-control"
          v-model="users"
        />
        <label for="users-input">Users</label>
      </div>

      <div>
        <input
          type="text"
          id="spawn-rate-input"
          class="form-control"
          v-model="spawnRate"
        />
        <label for="spawn-rate-input">Spawn rate</label>
      </div>

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
      <div>
        <input
          type="text"
          id="description-input"
          class="form-control"
          v-model="description"
        />
        <label for="description-input">Description</label>
      </div>
      <button type="button" id="start-btn" @click="start">Start</button>
    </form>
    
    -->

        <div v-for="test in reversedTests" :key="test.id" class="test-container" >
          <div class="uk-overflow-auto">
            <table class="uk-table uk-table-small uk-table-striped uk-table-responsive">
              <thead>
                <tr>
                  <th>{{test.id}}</th>
                  <th>{{test.info.users}}</th>
                  <th>{{test.info.spawn_rate}}</th>
                  <th>{{test.info.workers}}</th>
                  <th>{{test.info.host}}</th>
                  <th>{{test.info.time}}</th>
                  <th>{{test.status}}</th>
                  <th></th>
                  <th></th>
                  <th></th>
                  <th></th>
                </tr>
              </thead>
                            <thead>
                <tr>
                  <th>Type</th>
                  <th>Name</th>
                  <th>Requests</th>
                  <th>Failures</th>
                  <th>Med Res Time</th>
                  <th>Avg Res Time</th>
                  <th>Min Res Time</th>
                  <th>Max Res Time</th>
                  <th>Avg Content Size</th>
                  <th>Requests/s</th>
                  <th>Failures/s</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="result in test.results" :key="result">
                  <td>{{ result.type }}</td>
                  <td>{{ result.name }}</td>
                  <td>{{ result.request_count }}</td>
                  <td>{{ result.failure_count }}</td>
                  <td>{{ result.median_response_time.slice(0,6) }}</td>
                  <td>{{ result.avarage_response_time.slice(0,6) }}</td>
                  <td>{{ result.min_response_time.slice(0,6) }}</td>
                  <td>{{ result.max_response_time.slice(0,6) }}</td>
                  <td>{{ result.avarage_content_size.slice(0,6) }}</td>
                  <td>{{ result.requests_per_second.slice(0,6) }}</td>
                  <td>{{ result.failures_per_seconde.slice(0,6) }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        <button type="button" @click="stop(test.id)">Stop</button>
        <button type="button" @click="del(test.id)">Delete</button>
        </div>

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
    hideStartModal() {
      UIkit.modal(this.$refs["start-modal"]).hide();
    },
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
            this.hideStartModal();
          } else {
            console.log(data.error);
          }
        })
        .catch(() => {});
      return false;
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
        console.log(this.tests);
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