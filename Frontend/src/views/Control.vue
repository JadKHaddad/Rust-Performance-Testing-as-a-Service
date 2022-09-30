<template>
  <div>
    <div v-for="test in reversedTests" :key="test.id">
      <div class="text control-test-path">
        <router-link :to="{
          name: 'Project',
          params: { pid: test.info.project_id},
        }">
          {{ test.info.project_id }}
        </router-link>
        <span uk-icon="triangle-right"></span>
        <router-link :to="{
          name: 'Script',
          params: { pid: test.info.project_id, id: test.info.script_id },
        }">
          {{ test.info.script_id }}
        </router-link>
      </div>

      <Test :test="test" :darkTheme="darkTheme" @stop_me="stop(test.info)" @delete_me="del(test.info)"
        @restart_me="restart(test.info)" @download_me="download(test.info)" />
    </div>

  </div>
</template>

<script>
import Test from "@/components/Test.vue";
export default {
  name: "Control",
  components: {
    Test,
  },
  props: ["pid", "id", "deletedProject", "darkTheme"],
  data() {
    return {
      ws: null,
      tests: [],
    };
  },
  methods: { // TODO methods are so similar to Script.vue. Maybe we can merge them?
    connenctWebsocket() { //TODO: SAME AS SCRIPT.VUE
      var wsProtocol = "ws";
      if (location.protocol == 'https:') {
        wsProtocol = "wss";
      }
      this.ws = new WebSocket(
        `${wsProtocol}://${location.host}/api/master/subscribe/CONTROL/CONTROL`
      );
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
    restart(test_info) { //TODO: SAME AS SCRIPT.VUE
      fetch(`/api/worker/start_test/${test_info.project_id}/${test_info.script_id}`, {
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
    stop(test_info) { //TODO: SAME AS SCRIPT.VUE
      fetch(`/api/master/stop_test/${test_info.project_id}/${test_info.script_id}/${test_info.id}`, {
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
    del(test_info) { //TODO: SAME AS SCRIPT.VUE
      fetch(`/api/master/delete_test/${test_info.project_id}/${test_info.script_id}/${test_info.id}`, {
        method: "POST",
      })
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
            this.tests = this.tests.filter((test) => test.id !== test_info.id);
          } else {
            console.log(data.error);
          }
        })
        .catch();
    },
    download(test_info) { //TODO: SAME AS SCRIPT.VUE
      fetch(`/api/master/download_test/${test_info.project_id}/${test_info.script_id}/${test_info.id}`, {
        method: "GET",
      })
        .then((response) => response.blob())
        .then((blob) => {
          var objectUrl = URL.createObjectURL(blob);
          window.location.href = objectUrl;
        })
        .catch(() => { });
    },
  },
  computed: { //TODO: SAME AS SCRIPT.VUE
    reversedTests() {
      return this.tests.reverse();
    },
  },
  created() {
    this.connenctWebsocket();
    fetch(`/api/master/control`)
      .then((data) => data.json())
      .then((data) => {
        this.tests = data.content.tests;
      })
      .catch();
  },
  unmounted() { //TODO: SAME AS SCRIPT.VUE
    this.ws.close();
  },
};

</script>

<style>

</style>