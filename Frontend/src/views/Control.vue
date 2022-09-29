<template>
  <div>
    Control
    <Test v-for="test in reversedTests" :key="test.id" :test="test" :darkTheme="darkTheme" @stop_me="stop(test.info)" @delete_me="del(test.info)"
      @restart_me="restart(test.info)" @download_me="download(test.info)" />
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
    connenctWebsocket() {
      //TODO
    },
    restart(test_info) {
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
    stop(test_info) {
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
        .catch(() => {});
    },
    del(test_info) {
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
    download(test_info) {
      fetch(`/api/master/download_test/${test_info.project_id}/${test_info.script_id}/${test_info.id}`, {
        method: "GET",
      })
        .then((response) => response.blob())
        .then((blob) => {
          var objectUrl = URL.createObjectURL(blob);
          window.location.href = objectUrl;
        })
        .catch(() => {});
    },
  },
  computed: {
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
};

</script>

<style>
</style>