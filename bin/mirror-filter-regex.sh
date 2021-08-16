#/usr/local/bin/bash
for PACKAGE in $1; do
  PACKAGES_NEW=$(pkg_info -f $PACKAGE | grep '@depend' | cut -f 3 -d ':' | sed -E "s/[0-9\-\.vp]+$//g" | sed "s/\-$//g" | tr '\n' ' ')
  PACKAGE=$(echo $PACKAGE | sed "s/\-\-//g")
  PACKAGES="$PACKAGES $PACKAGES_NEW $PACKAGE"
done
PACKAGES=$(echo $PACKAGES | sed "s/ /\|/g")
REGEX=".*\/($PACKAGES)[0-9\.pv\-]*\.tgz$"
echo $REGEX
