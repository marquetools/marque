<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00002">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00002][Error] For every attribute in the ISM namespace that is used in a document, a non-null value must be present.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For each element which defines an attribute in the ISM namespace, this rule ensures that each attribute in the ISM namespace 
        is specified with a non-whitespace value.
    </sch:p>
    <sch:rule id="ISM-ID-00002-R1" context="*[@ism:*]">
        <sch:assert test="every $attribute in @ism:* satisfies normalize-space(string($attribute))" flag="error" role="error">
            [ISM-ID-00002][Error] For every attribute in the ISM namespace that is used in a document, a non-null value must be present.
        </sch:assert>
    </sch:rule>
</sch:pattern>