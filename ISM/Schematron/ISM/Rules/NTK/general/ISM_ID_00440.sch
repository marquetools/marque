<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00048 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00440">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"  ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00440][Error] ntk:AccessPolicy, ntk:ProfileDes, and ntk:AccessProfileValue are required to have text content.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        ntk:AccessPolicy, ntk:ProfileDes, and ntk:AccessProfileValue are required to have text content.
    </sch:p>
    <sch:rule id="ISM-ID-00440-R1" context="ntk:AccessPolicy | ntk:ProfileDes | ntk:AccessProfileValue">
        <sch:assert test="not(empty(text()))" flag="error" role="error">
            [ISM-ID-00440][Error] ntk:AccessPolicy, ntk:ProfileDes, and ntk:AccessProfileValue are required to have text content.
        </sch:assert>
    </sch:rule>
</sch:pattern>
