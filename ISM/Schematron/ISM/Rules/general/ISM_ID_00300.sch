<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00300">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00300][Warning] @ism:DESVersion attributes SHOULD be specified as revision 202111.202211 (Revision:2021-NOVr2022-NOV) with an optional extension.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This rule supports extending the version identifier with an optional trailing hyphen
        and up to 23 additional characters. The version must match the regular expression
        “^202111.202211(-.{1,23})?$".
    </sch:p>
    <sch:rule id="ISM-ID-00300-R1" context="*[@ism:DESVersion]">
        <sch:assert test="matches(@ism:DESVersion,'^202111.202211(-.{1,23})?$')" flag="warning" role="warning">
            [ISM-ID-00300][Warning] @ism:DESVersion attributes SHOULD be specified as revision 202111.202211 (Revision:2021-NOVr2022-NOV) with an optional extension.
            The value provided was: <sch:value-of select="./@ism:DESVersion"/>
        </sch:assert>
    </sch:rule>
</sch:pattern>
