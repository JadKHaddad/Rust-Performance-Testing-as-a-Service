<template>
  <div>
    <div  class="uk-grid-small uk-child-width-1-3@s uk-flex-center uk-text-center" uk-grid>
      <div class="uk-width-auto@m">
        <button class="uk-button uk-button-primary" type="button" uk-toggle="target: #start-modal">
          Start
        </button>
      </div>	
      <div class="uk-width-auto@m">
        <button class="uk-button uk-button-primary" type="button" @click="check_script">
          Check
        </button>
      </div>
      <div class="uk-width-auto@m">
        <button class="uk-button uk-button-danger" type="button" @click="stop_all">
          Stop All
        </button>
      </div>
      </div>
    <div id="start-modal" uk-modal ref="start-modal">
      <div class="uk-modal-dialog uk-modal-body" :class="{
      dark: darkTheme,
    }">
        <form>
          <div class="uk-margin">
            <input class="uk-input" type="text" placeholder="Users" v-model="users" />
          </div>
          <div class="uk-margin">
            <input class="uk-input" type="text" placeholder="Spawn rate" v-model="spawnRate" />
          </div>
          <div class="uk-margin">
            <input class="uk-input" type="text" placeholder="Workers" v-model="workers" />
          </div>
          <div class="uk-margin">
            <label class="uk-form-label text" for="host">This will overwrite all hosts in your file</label>
            <input id="host" class="uk-input" type="text" placeholder="Host" v-model="host" />
          </div>
          <div class="uk-margin">
            <label class="uk-form-label text" for="time">If time is not set, the test will not stop automatically</label>
            <input id="time" class="uk-input" type="text" placeholder="Time is seconds" v-model="time" />
          </div>
          <div class="uk-margin">
            <input class="uk-input" type="text" placeholder="Description" v-model="description" />
          </div>
          <button class="uk-button uk-button-primary uk-margin-small-right" type="button" @click="start_from_modal">
            Start
          </button>
        </form>
      </div>
    </div>

    <h3 class="text" >Project: {{ pid }} | Script: {{ id }}</h3>

    <Test v-for="test in reversedTests" :key="test.id" :test="test" :darkTheme="darkTheme" @stop_me="stop(test.id)" @delete_me="del(test.id)"
      @restart_me="restart(test.info)" @download_me="download(test.id)" />
  </div>
</template>

<script>
import Test from "@/components/Test.vue";
export default {
  name: "Script",
  components: {
    Test,
  },
  props: ["pid", "id", "deletedProject", "darkTheme"],
  watch: {
    deletedProject: function () {
      if (this.pid == this.deletedProject.id) {
        this.$router.replace({ name: "Home" });
      }
    },
  },
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
      this.ws.onopen = () => { };
      this.ws.onclose = () => { };
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
              test.last_history = incomingTest.last_history;
            }
          }
          return;
        }
        if (event_type === "TEST_STOPPED") {
          const id = data.event.id;
          let test = this.tests.find((test) => test.id === id);
          if (test) {
            test.status = 1;
          }
        }
        if (event_type === "TEST_STARTED") {
          const new_test = data.event;
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
    start(test_info) {
      fetch(`/api/worker/start_test/${this.pid}/${this.id}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(test_info),
      })
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
            // ws will add the new test
            // let test = data.content;
            // this.tests.push(test);
            // this.ws.send(
            //   JSON.stringify({
            //     event_type: "TEST_STARTED",
            //     event: {
            //       test: test,
            //     },
            //   })
            // );
            this.hideStartModal();
          } else {
            console.log(data.error);
            //notify
            this.$root.notification(
              `Error running test: ${data.error}`,
              "danger",
              0
            );
          }
        })
        .catch(() => { });
    },
    start_from_modal() {
      const test_info = {
        users: parseInt(this.users),
        spawn_rate: parseInt(this.spawnRate),
        workers: parseInt(this.workers),
        host: this.host,
        time: parseInt(this.time),
        description: this.description,
      };
      this.start(test_info);
      return false;
    },
    stop(test_id) {
      fetch(`/api/master/stop_test/${this.pid}/${this.id}/${test_id}`, {
        method: "POST",
      })
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
            let test = this.tests.find((test) => test.id === test_id);
            if (test) {
              test.status = 1;
            }
          } else {
            console.log(data.error);
          }
        })
        .catch(() => { });
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
        .catch(() => { });
    },
    restart(test_info) {
      this.start(test_info);
      return false;
    },
    download(test_id) {
      fetch(`/api/master/download_test/${this.pid}/${this.id}/${test_id}`, {
        method: "GET",
      })
        .then((response) => response.blob())
        .then((blob) => {
          var objectUrl = URL.createObjectURL(blob);
          window.location.href = objectUrl;
        })
        .catch(() => { });
    },
    stop_all() {
      fetch(`/api/master/stop_script/${this.pid}/${this.id}`, {
        method: "POST",
      })
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
          } else {
            console.log(data.error);
          }
        })
        .catch(() => { });
      return false;
    },
    check_script() {
      this.$router.push({ name: 'Check', params: { pid: this.pid, id: this.id } })
    }
  },
  created() {
    this.connenctWebsocket();
    fetch(`/api/master/tests/${this.pid}/${this.id}`)
      .then((data) => data.json())
      .then((data) => {
        this.tests = data.content.tests;
        const config = data.content.config
        this.users = config.users;
        this.spawnRate = config.spawn_rate;
        this.workers = config.workers;
        this.host = config.host;
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