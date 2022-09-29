<template>
  <div>
    Control
    <Test v-for="test in reversedTests" :key="test.id" :test="test" :darkTheme="darkTheme" @stop_me="stop(test.id)" @delete_me="del(test.id)"
      @restart_me="restart(test.info)" @download_me="download(test.id)" />
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
  methods: {
    connenctWebsocket() {}
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