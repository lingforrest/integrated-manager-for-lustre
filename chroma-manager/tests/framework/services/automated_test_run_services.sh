#!/bin/sh

set -ex

[ -r localenv ] && . localenv

CHROMA_DIR=${CHROMA_DIR:-"$PWD/chroma/"}
CLUSTER_CONFIG=${CLUSTER_CONFIG:-"$CHROMA_DIR/chroma-manager/tests/framework/services/cluster_config.json"}

eval $(python $CHROMA_DIR/chroma-manager/tests/utils/json_cfg2sh.py "$CLUSTER_CONFIG")

MEASURE_COVERAGE=${MEASURE_COVERAGE:-false}
TESTS=${TESTS:-"tests/services/"}
EPEL_SOURCE=${EPEL_SOURCE:-"baseurl=http://cobbler/cobbler/repo_mirror/EPEL-6-x86_64/"}  # For a dev env, use mirrorlist=https://mirrors.fedoraproject.org/metalink?repo=epel-6&arch=$basearch

echo "Beginning installation and setup..."

# Install and setup chroma manager
ssh root@$CHROMA_MANAGER <<EOF
set -ex
# Install non-python/pipable dependencies
cat <<EOC > /etc/yum.repos.d/epel.repo
[epel]
name=epel
$EPEL_SOURCE
enabled=1
priority=1
gpgcheck=0
sslverify=0
EOC
yum makecache
yum install -y git python-virtualenv python-setuptools python-devel gcc make graphviz-devel postgresql-server postgresql-devel rabbitmq-server mod_wsgi mod_ssl telnet python-ethtool erlang-inets npm patch httpd-devel

# Create a user so we can run chroma as non-root
useradd chromatest
su chromatest
mkdir -p ~/.ssh
touch ~/.ssh/authorized_keys
exit
cat .ssh/id_rsa.pub >> /home/chromatest/.ssh/authorized_keys
scp .ssh/* chromatest@$CHROMA_MANAGER:.ssh/

# Configure rabbitmq
chkconfig rabbitmq-server on

# Some witch-craft to run rabbitmq as non-root in linux
echo "%rabbitmq ALL=(ALL) NOPASSWD: /usr/sbin/rabbitmqctl"  > /etc/sudoers.d/rabbitmqctl
chmod 440 /etc/sudoers.d/rabbitmqctl
sed -i "s/Defaults    requiretty/# Defaults    requiretty/g" /etc/sudoers
sed -i "s/rabbitmq:x:\([0-9]*\):[a-z]*/rabbitmq:x:\1:chromatest/g" /etc/group
# End witchcraft

export PATH=$PATH:/usr/lib/rabbitmq/bin/
rabbitmq-plugins enable rabbitmq_management
service rabbitmq-server start &> rabbitmq_startup.log
cat rabbitmq_startup.log


# Testing rabbitmq and wait to be up and running
COUNTER=0
MAX_TRIALS=15
rabbitmqctl status
while [[ \$? -ne 0 && \$COUNTER -ne \$MAX_TRIALS ]];
    do
    sleep 2
    let COUNTER=COUNTER+1
    rabbitmqctl status
done;
echo "counter: \$COUNTER, trials: \$MAX_TRIALS"
[[ \$COUNTER -eq \$MAX_TRIALS ]] && { echo "RabbitMQ cannot be started!"; exit 1; }

# Testing rabbitmq internal messaging
curl http://localhost:15672/cli/rabbitmqadmin > \$HOME/rabbitmqadmin
chmod u+x \$HOME/rabbitmqadmin
\$HOME/rabbitmqadmin declare queue name=test-queue durable=false
\$HOME/rabbitmqadmin publish exchange=amq.default routing_key=test-queue payload="test_message"

COUNTER=0
grep_msg="\$HOME/rabbitmqadmin get queue=test-queue requeue=false | grep test_message"
while [[ \$(eval \$grep_msg) == "" && \$COUNTER -ne \$MAX_TRIALS ]];
    do
    sleep 2
    let COUNTER=COUNTER+1
done;
echo "counter: \$COUNTER, trials: \$MAX_TRIALS"
[[ \$COUNTER -eq \$MAX_TRIALS ]] && { echo "RabbitMQ cannot receive messages!"; exit 1; }


# Configure postgres
chkconfig postgresql on
service postgresql initdb
service postgresql start
# TODO: sleeping is racy.  should check for up-ness, not just assume it
#       will happen within 5 seconds
sleep 5  # Unfortunately postgresql start seems to return before its truly up and ready for business
su postgres -c 'createuser -R -S -d chroma'
su postgres -c 'createdb -O chroma chroma'
sed -i -e '/local[[:space:]]\+all/i\
local   all         chroma                            trust' /var/lib/pgsql/data/pg_hba.conf
service postgresql restart
EOF

virtualenv --relocatable .
tar -czf ../chroma_test_env.tgz .
scp ../chroma_test_env.tgz chromatest@$CHROMA_MANAGER:~

ssh chromatest@$CHROMA_MANAGER <<EOF
set -ex
mkdir -p chroma_test_env
cd chroma_test_env
tar -xzf ~/chroma_test_env.tgz
virtualenv --no-site-packages .
source bin/activate
cd chroma/chroma-manager/

if $MEASURE_COVERAGE; then
  cat <<EOC > /home/chromatest/chroma_test_env/chroma/chroma-manager/.coveragerc
[run]
data_file = /var/tmp/.coverage
parallel = True
source = /home/chromatest/chroma_test_env/chroma
EOC
  cat <<EOC  > /home/chromatest/chroma_test_env/lib/python2.6/site-packages/sitecustomize.py
import coverage
cov = coverage.coverage(config_file='/home/chromatest/chroma_test_env/chroma/chroma-manager/.coveragerc', auto_data=True)
cov.start()
cov._warn_no_data = False
cov._warn_unimported_source = False
EOC
fi

cd ~/chroma_test_env/chroma/chroma-manager

# Enable DEBUG logging
cat <<"EOF1" > local_settings.py
import logging
LOG_LEVEL = logging.DEBUG
EOF1

make develop

set +e
echo "Begin running tests..."
PYTHONPATH=. nosetests --verbosity=2 --with-xunit --xunit-file /home/chromatest/test_report.xml $TESTS
ls /home/chromatest/

EOF
test_status=$?

echo "End running tests."

set +e
cd $CHROMA_DIR/../..
mkdir -p test_reports
scp chromatest@$CHROMA_MANAGER:test_report.xml ./test_reports/
mkdir -p test_logs
scp chromatest@$CHROMA_MANAGER:chroma_test_env/chroma/chroma-manager/*.log ./test_logs/
scp root@$CHROMA_MANAGER:/var/log/messages ./test_logs/
if $MEASURE_COVERAGE; then
  mkdir -p coverage_reports
  ssh chromatest@$CHROMA_MANAGER <<EOF
    set -x
    source chroma_test_env/bin/activate
    cd /var/tmp/
    coverage combine
EOF
  scp chromatest@$CHROMA_MANAGER:/var/tmp/.coverage ./.coverage.raw
fi
set -e

if [ $test_status -ne 0 ]; then
    echo "AUTOMATED TEST RUN FAILED"
fi

exit $test_status