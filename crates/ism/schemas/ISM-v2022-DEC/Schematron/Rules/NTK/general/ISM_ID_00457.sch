<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00004 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00457">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00457][Error] Every attribute in the NTK namespace must be specified with a non-whitespace value.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For each element which specifies an attribute in the NTK namespace, this rule ensures that all attributes
        in the NTK namespace contain a non-whitespace value.
    </sch:p>
    <sch:rule id="ISM-ID-00457-R1" context="*[@ntk:*]">
        <sch:assert test="every $attribute in @ntk:* satisfies               normalize-space(string($attribute))" flag="error" role="error">
            [ISM-ID-00457][Error] Every attribute in the document must be specified with a non-whitespace value.
        </sch:assert>
    </sch:rule>
</sch:pattern>
