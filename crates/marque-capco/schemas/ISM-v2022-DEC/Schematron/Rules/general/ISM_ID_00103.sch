<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00103">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00103][Error] At least one element must have attribute @ism:resourceElement specified with a value of [true].
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For the document, this rule ensures that at least one element specifies attribute @ism:resourceElement with a value of [true].
    </sch:p>
    <sch:rule id="ISM-ID-00103-R1" context="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]">
        <sch:assert test="some $token in //*[(@ism:*)] satisfies               $token/@ism:resourceElement=true()" flag="error" role="error">
        	[ISM-ID-00103][Error] At least one element must have attribute @ism:resourceElement specified with a value of [true].
        </sch:assert>
    </sch:rule>
</sch:pattern>