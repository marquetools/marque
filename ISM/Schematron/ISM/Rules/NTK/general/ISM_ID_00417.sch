<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00024 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00417">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00417][Error] If there is a Profile DES specified, then there must be at least
        one ntk:AccessProfileValue.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        When ntk:ProfileDes exists, make sure there is also a following sibling ntk:AccessProfileValue.
    </sch:p>
    <sch:rule id="ISM-ID-00417-R1" context="ntk:ProfileDes">
        <sch:assert test="following-sibling::ntk:AccessProfileValue" flag="error" role="error">
            [ISM-ID-00417][Error] If there is a Profile DES specified, then there must be at least
            one ntk:AccessProfileValue.
        </sch:assert>
    </sch:rule>
</sch:pattern>
