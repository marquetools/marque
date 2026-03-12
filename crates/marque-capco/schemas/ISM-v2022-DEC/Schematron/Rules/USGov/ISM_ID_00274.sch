<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00274">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00274][Error] All @ism:createDate attributes must be a Date without a timezone.
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		For all elements which contain a @ism:createDate attribute, this rule ensures that
		the createDate value matches the pattern defined for type Date without timezone information.
		The value must conform to the Regex ‘[0-9]{4}-[0-9]{2}-[0-9]{2}$’
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeNote">
		The first assert in this rule is not able to be failed in unit tests. If
		the @ism:createDate does not conform to type Date, schematron fails when defining global
		variables before any rules are fired. The first assert is included as a normative statement
		of the requirement that the attribute be a Date type. The rule can fail the second assert,
		which ensures there is no timezone info.
	</sch:p>
	<sch:rule id="ISM-ID-00274-R1" context="*[@ism:createDate]">
		<sch:assert test="util:meetsType(string(@ism:createDate), $DatePattern)" flag="error" role="error">
			[ISM-ID-00274][Error] All @ism:createDate attribute values must be of type Date. 
		</sch:assert>
		<sch:assert test="matches(@ism:createDate, '[0-9]{4}-[0-9]{2}-[0-9]{2}$')" flag="error" role="error">
			[ISM-ID-00274][Error] All @ism:createDate attribute values must not have any timezone information specified. 
		</sch:assert>
	</sch:rule>
</sch:pattern>