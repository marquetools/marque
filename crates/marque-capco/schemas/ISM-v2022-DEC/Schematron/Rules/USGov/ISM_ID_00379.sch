<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00379">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00379][Error] All ISM @ism:declassDate attributes must be a Date without a timezone.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For all elements which contain a @ism:declassDate attribute, this rule ensures that
        the declassDate value matches the pattern defined for type Date without timezone information.
        The value must conform to the Regex ‘[0-9]{4}-[0-9]{2}-[0-9]{2}$’
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeNote">
        The first assert in this rule is not able to be failed in unit tests. If
        the declassDate does not conform to type Date, schematron fails when defining global
        variables before any rules are fired. The first assert is included as a normative statement
        of the requirement that the attribute be a Date type. The rule can fail the second assert,
        which ensures there is no timezone info.
    </sch:p>
    <sch:rule id="ISM-ID-00379-R1" context="*[@ism:declassDate]">
        <sch:assert test="util:meetsType(@ism:declassDate, $DatePattern)" flag="error" role="error">
            [ISM-ID-00379][Error] All @ism:declassDate attribute values must be of type Date. 
        </sch:assert>
        <sch:assert test="matches(string(@ism:declassDate), '[0-9]{4}-[0-9]{2}-[0-9]{2}$')" flag="error" role="error">
            [ISM-ID-00379][Error] All @ism:declassDate attribute values must not have any timezone
            information specified. 
        </sch:assert>
    </sch:rule>
</sch:pattern>